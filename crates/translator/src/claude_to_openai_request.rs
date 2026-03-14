use prism_types::error::ProxyError;
use serde_json::{Value, json};

/// Translate a Claude Messages API request body to an OpenAI Chat Completions request body.
pub fn translate_request(
    model: &str,
    raw_json: &[u8],
    stream: bool,
) -> Result<Vec<u8>, ProxyError> {
    let req: Value = serde_json::from_slice(raw_json)?;

    let mut messages = Vec::new();

    // Extract system prompt
    if let Some(system) = req.get("system") {
        let system_text = match system {
            Value::String(s) => s.clone(),
            Value::Array(blocks) => blocks
                .iter()
                .filter_map(|b| {
                    if b.get("type").and_then(|t| t.as_str()) == Some("text") {
                        b.get("text")
                            .and_then(|t| t.as_str())
                            .map(|s| s.to_string())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n"),
            _ => String::new(),
        };
        if !system_text.is_empty() {
            messages.push(json!({"role": "system", "content": system_text}));
        }
    }

    // Convert messages
    if let Some(msg_array) = req.get("messages").and_then(|m| m.as_array()) {
        for msg in msg_array {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
            let content = msg.get("content");

            match role {
                "user" => {
                    if let Some(content) = content {
                        match content {
                            Value::String(s) => {
                                messages.push(json!({"role": "user", "content": s}));
                            }
                            Value::Array(blocks) => {
                                // Check if this contains tool_result blocks
                                let has_tool_results = blocks.iter().any(|b| {
                                    b.get("type").and_then(|t| t.as_str()) == Some("tool_result")
                                });
                                if has_tool_results {
                                    for block in blocks {
                                        if block.get("type").and_then(|t| t.as_str())
                                            == Some("tool_result")
                                        {
                                            let tool_use_id = block
                                                .get("tool_use_id")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("");
                                            let result_content = match block.get("content") {
                                                Some(Value::String(s)) => s.clone(),
                                                Some(Value::Array(parts)) => parts
                                                    .iter()
                                                    .filter_map(|p| {
                                                        p.get("text")
                                                            .and_then(|t| t.as_str())
                                                            .map(String::from)
                                                    })
                                                    .collect::<Vec<_>>()
                                                    .join(""),
                                                _ => String::new(),
                                            };
                                            messages.push(json!({
                                                "role": "tool",
                                                "tool_call_id": tool_use_id,
                                                "content": result_content,
                                            }));
                                        }
                                    }
                                } else {
                                    let openai_parts = convert_user_content_blocks(blocks);
                                    if openai_parts.len() == 1
                                        && openai_parts[0].get("type").and_then(|t| t.as_str())
                                            == Some("text")
                                    {
                                        messages.push(json!({
                                            "role": "user",
                                            "content": openai_parts[0]["text"]
                                        }));
                                    } else {
                                        messages.push(json!({
                                            "role": "user",
                                            "content": openai_parts
                                        }));
                                    }
                                }
                            }
                            _ => {
                                messages.push(json!({"role": "user", "content": ""}));
                            }
                        }
                    }
                }
                "assistant" => {
                    if let Some(Value::Array(blocks)) = content {
                        let (text_parts, tool_calls, thinking_parts) =
                            convert_assistant_content_blocks(blocks);

                        let mut msg = json!({"role": "assistant"});

                        // Add reasoning_content if thinking blocks present
                        if !thinking_parts.is_empty() {
                            msg["reasoning_content"] = Value::String(thinking_parts.join("\n"));
                        }

                        let content_str = text_parts.join("");
                        if content_str.is_empty() && !tool_calls.is_empty() {
                            msg["content"] = Value::Null;
                        } else {
                            msg["content"] = Value::String(content_str);
                        }

                        if !tool_calls.is_empty() {
                            msg["tool_calls"] = Value::Array(tool_calls);
                        }

                        messages.push(msg);
                    } else if let Some(Value::String(s)) = content {
                        messages.push(json!({"role": "assistant", "content": s}));
                    }
                }
                _ => {}
            }
        }
    }

    // Build OpenAI request
    let mut openai_req = json!({
        "model": model,
        "messages": messages,
    });

    if stream {
        openai_req["stream"] = Value::Bool(true);
    }

    // max_tokens
    if let Some(max_tokens) = req.get("max_tokens") {
        openai_req["max_tokens"] = max_tokens.clone();
    }

    // temperature
    if let Some(temp) = req.get("temperature") {
        openai_req["temperature"] = temp.clone();
    }

    // top_p
    if let Some(top_p) = req.get("top_p") {
        openai_req["top_p"] = top_p.clone();
    }

    // stop_sequences → stop
    if let Some(stop) = req.get("stop_sequences") {
        openai_req["stop"] = stop.clone();
    }

    // tools → OpenAI tools format
    if let Some(tools) = req.get("tools").and_then(|t| t.as_array()) {
        let openai_tools: Vec<Value> = tools
            .iter()
            .filter_map(|tool| {
                let name = tool.get("name")?.as_str()?;
                let description = tool
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("");
                let parameters = tool
                    .get("input_schema")
                    .cloned()
                    .unwrap_or(json!({"type": "object", "properties": {}}));
                Some(json!({
                    "type": "function",
                    "function": {
                        "name": name,
                        "description": description,
                        "parameters": parameters,
                    }
                }))
            })
            .collect();
        if !openai_tools.is_empty() {
            openai_req["tools"] = Value::Array(openai_tools);
        }
    }

    // tool_choice
    if let Some(tc) = req.get("tool_choice") {
        openai_req["tool_choice"] = convert_tool_choice(tc);
    }

    // thinking → reasoning_effort (best-effort mapping)
    if let Some(thinking) = req.get("thinking")
        && thinking.get("type").and_then(|t| t.as_str()) == Some("enabled")
        && let Some(budget) = thinking.get("budget_tokens").and_then(|b| b.as_u64())
    {
        let effort = if budget <= 1024 {
            "low"
        } else if budget <= 4096 {
            "medium"
        } else {
            "high"
        };
        openai_req["reasoning_effort"] = Value::String(effort.to_string());
    }

    serde_json::to_vec(&openai_req).map_err(|e| ProxyError::Translation(e.to_string()))
}

fn convert_user_content_blocks(blocks: &[Value]) -> Vec<Value> {
    let mut parts = Vec::new();
    for block in blocks {
        let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match block_type {
            "text" => {
                let text = block.get("text").and_then(|t| t.as_str()).unwrap_or("");
                parts.push(json!({"type": "text", "text": text}));
            }
            "image" => {
                if let Some(source) = block.get("source") {
                    let source_type = source.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    match source_type {
                        "base64" => {
                            let media_type = source
                                .get("media_type")
                                .and_then(|m| m.as_str())
                                .unwrap_or("image/png");
                            let data = source.get("data").and_then(|d| d.as_str()).unwrap_or("");
                            let url = format!("data:{media_type};base64,{data}");
                            parts.push(json!({
                                "type": "image_url",
                                "image_url": {"url": url}
                            }));
                        }
                        "url" => {
                            let url = source.get("url").and_then(|u| u.as_str()).unwrap_or("");
                            parts.push(json!({
                                "type": "image_url",
                                "image_url": {"url": url}
                            }));
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    parts
}

fn convert_assistant_content_blocks(blocks: &[Value]) -> (Vec<String>, Vec<Value>, Vec<String>) {
    let mut text_parts = Vec::new();
    let mut tool_calls = Vec::new();
    let mut thinking_parts = Vec::new();
    let mut tc_index = 0u32;

    for block in blocks {
        let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match block_type {
            "text" => {
                if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                    text_parts.push(text.to_string());
                }
            }
            "thinking" => {
                if let Some(text) = block.get("thinking").and_then(|t| t.as_str())
                    && !text.is_empty()
                {
                    thinking_parts.push(text.to_string());
                }
            }
            "tool_use" => {
                let id = block.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let name = block.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let input = block.get("input").cloned().unwrap_or(json!({}));
                let arguments = serde_json::to_string(&input).unwrap_or_default();

                tool_calls.push(json!({
                    "id": id,
                    "type": "function",
                    "index": tc_index,
                    "function": {
                        "name": name,
                        "arguments": arguments,
                    }
                }));
                tc_index += 1;
            }
            _ => {}
        }
    }

    (text_parts, tool_calls, thinking_parts)
}

fn convert_tool_choice(tc: &Value) -> Value {
    if let Some(obj) = tc.as_object() {
        let tc_type = obj.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match tc_type {
            "none" => json!("none"),
            "auto" => json!("auto"),
            "any" => json!("required"),
            "tool" => {
                if let Some(name) = obj.get("name").and_then(|n| n.as_str()) {
                    json!({"type": "function", "function": {"name": name}})
                } else {
                    json!("auto")
                }
            }
            _ => json!("auto"),
        }
    } else {
        json!("auto")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn translate(req: Value, stream: bool) -> Value {
        let raw = serde_json::to_vec(&req).unwrap();
        let result = translate_request("gpt-4o", &raw, stream).unwrap();
        serde_json::from_slice(&result).unwrap()
    }

    #[test]
    fn test_basic_text_message() {
        let req = json!({
            "model": "claude-3-5-sonnet-20241022",
            "messages": [{"role": "user", "content": "Hello"}],
            "max_tokens": 1024
        });
        let result = translate(req, false);
        assert_eq!(result["model"], "gpt-4o");
        assert_eq!(result["messages"][0]["role"], "user");
        assert_eq!(result["messages"][0]["content"], "Hello");
        assert_eq!(result["max_tokens"], 1024);
        assert!(result.get("stream").is_none());
    }

    #[test]
    fn test_stream_flag() {
        let req = json!({
            "model": "claude-3-5-sonnet-20241022",
            "messages": [{"role": "user", "content": "Hi"}],
            "max_tokens": 100
        });
        let result = translate(req, true);
        assert_eq!(result["stream"], true);
    }

    #[test]
    fn test_system_string() {
        let req = json!({
            "model": "claude-3-5-sonnet-20241022",
            "system": "You are helpful.",
            "messages": [{"role": "user", "content": "Hi"}],
            "max_tokens": 100
        });
        let result = translate(req, false);
        assert_eq!(result["messages"][0]["role"], "system");
        assert_eq!(result["messages"][0]["content"], "You are helpful.");
        assert_eq!(result["messages"][1]["role"], "user");
    }

    #[test]
    fn test_system_blocks() {
        let req = json!({
            "model": "claude-3-5-sonnet-20241022",
            "system": [{"type": "text", "text": "Rule A"}, {"type": "text", "text": "Rule B"}],
            "messages": [{"role": "user", "content": "Hi"}],
            "max_tokens": 100
        });
        let result = translate(req, false);
        assert_eq!(result["messages"][0]["role"], "system");
        assert_eq!(result["messages"][0]["content"], "Rule A\nRule B");
    }

    #[test]
    fn test_assistant_with_tool_use() {
        let req = json!({
            "model": "claude-3-5-sonnet-20241022",
            "messages": [
                {"role": "user", "content": "Weather?"},
                {"role": "assistant", "content": [
                    {"type": "tool_use", "id": "toolu_1", "name": "get_weather", "input": {"city": "SF"}}
                ]},
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "toolu_1", "content": "72°F"}
                ]}
            ],
            "max_tokens": 100
        });
        let result = translate(req, false);
        // Assistant should have tool_calls
        let assistant = &result["messages"][1];
        assert_eq!(assistant["role"], "assistant");
        assert_eq!(assistant["content"], Value::Null);
        let tcs = assistant["tool_calls"].as_array().unwrap();
        assert_eq!(tcs[0]["id"], "toolu_1");
        assert_eq!(tcs[0]["function"]["name"], "get_weather");

        // Tool result should be a tool message
        let tool = &result["messages"][2];
        assert_eq!(tool["role"], "tool");
        assert_eq!(tool["tool_call_id"], "toolu_1");
        assert_eq!(tool["content"], "72°F");
    }

    #[test]
    fn test_assistant_with_thinking() {
        let req = json!({
            "model": "claude-sonnet-4-5-20250514",
            "messages": [
                {"role": "user", "content": "Think hard"},
                {"role": "assistant", "content": [
                    {"type": "thinking", "thinking": "Let me analyze..."},
                    {"type": "text", "text": "The answer is 42."}
                ]}
            ],
            "max_tokens": 1000
        });
        let result = translate(req, false);
        let assistant = &result["messages"][1];
        assert_eq!(assistant["reasoning_content"], "Let me analyze...");
        assert_eq!(assistant["content"], "The answer is 42.");
    }

    #[test]
    fn test_tools_conversion() {
        let req = json!({
            "model": "claude-3-5-sonnet-20241022",
            "messages": [{"role": "user", "content": "Hi"}],
            "max_tokens": 100,
            "tools": [{
                "name": "get_weather",
                "description": "Get weather for a city",
                "input_schema": {
                    "type": "object",
                    "properties": {"city": {"type": "string"}},
                    "required": ["city"]
                }
            }]
        });
        let result = translate(req, false);
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["type"], "function");
        assert_eq!(tools[0]["function"]["name"], "get_weather");
        assert_eq!(
            tools[0]["function"]["parameters"]["properties"]["city"]["type"],
            "string"
        );
    }

    #[test]
    fn test_tool_choice_conversion() {
        let req = json!({
            "model": "claude-3-5-sonnet-20241022",
            "messages": [{"role": "user", "content": "Hi"}],
            "max_tokens": 100,
            "tool_choice": {"type": "any"}
        });
        let result = translate(req, false);
        assert_eq!(result["tool_choice"], "required");
    }

    #[test]
    fn test_tool_choice_specific() {
        let req = json!({
            "model": "claude-3-5-sonnet-20241022",
            "messages": [{"role": "user", "content": "Hi"}],
            "max_tokens": 100,
            "tool_choice": {"type": "tool", "name": "get_weather"}
        });
        let result = translate(req, false);
        assert_eq!(result["tool_choice"]["function"]["name"], "get_weather");
    }

    #[test]
    fn test_stop_sequences() {
        let req = json!({
            "model": "claude-3-5-sonnet-20241022",
            "messages": [{"role": "user", "content": "Hi"}],
            "max_tokens": 100,
            "stop_sequences": ["END", "STOP"]
        });
        let result = translate(req, false);
        assert_eq!(result["stop"], json!(["END", "STOP"]));
    }

    #[test]
    fn test_thinking_to_reasoning_effort() {
        let req = json!({
            "model": "claude-sonnet-4-5-20250514",
            "messages": [{"role": "user", "content": "Hi"}],
            "max_tokens": 8192,
            "thinking": {"type": "enabled", "budget_tokens": 10000}
        });
        let result = translate(req, false);
        assert_eq!(result["reasoning_effort"], "high");
    }

    #[test]
    fn test_user_image_content() {
        let req = json!({
            "model": "claude-3-5-sonnet-20241022",
            "messages": [{"role": "user", "content": [
                {"type": "text", "text": "Describe:"},
                {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "iVBOR..."}}
            ]}],
            "max_tokens": 100
        });
        let result = translate(req, false);
        let content = result["messages"][0]["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[1]["type"], "image_url");
        assert!(
            content[1]["image_url"]["url"]
                .as_str()
                .unwrap()
                .starts_with("data:image/png;base64,")
        );
    }

    #[test]
    fn test_temperature_and_top_p() {
        let req = json!({
            "model": "claude-3-5-sonnet-20241022",
            "messages": [{"role": "user", "content": "Hi"}],
            "max_tokens": 100,
            "temperature": 0.7,
            "top_p": 0.9
        });
        let result = translate(req, false);
        assert_eq!(result["temperature"], 0.7);
        assert_eq!(result["top_p"], 0.9);
    }
}
