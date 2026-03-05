use prism_core::error::ProxyError;
use serde_json::{Value, json};

pub fn translate_request(
    model: &str,
    raw_json: &[u8],
    stream: bool,
) -> Result<Vec<u8>, ProxyError> {
    let mut req: Value = serde_json::from_slice(raw_json)?;

    // 1. Extract system messages from messages array
    let system_text = extract_system_messages(&mut req);

    // 2. Convert messages to Claude format
    let messages = convert_messages(&req)?;

    // 3. Convert tools
    let tools = convert_tools(&req);

    // 4. Determine max_tokens
    let max_tokens = req
        .get("max_tokens")
        .or_else(|| req.get("max_completion_tokens"))
        .and_then(|v| v.as_u64())
        .unwrap_or(8192);

    // 5. Convert stop sequences
    let stop_sequences = convert_stop_sequences(&req);

    // Build Claude request
    let mut claude_req = json!({
        "model": model,
        "messages": messages,
        "max_tokens": max_tokens,
    });

    if !system_text.is_empty() {
        claude_req["system"] = Value::String(system_text);
    }

    if let Some(temp) = req.get("temperature") {
        claude_req["temperature"] = temp.clone();
    }
    if let Some(top_p) = req.get("top_p") {
        claude_req["top_p"] = top_p.clone();
    }
    if let Some(tools) = tools {
        claude_req["tools"] = tools;
    }
    if let Some(stop) = stop_sequences {
        claude_req["stop_sequences"] = stop;
    }
    if stream {
        claude_req["stream"] = Value::Bool(true);
    }

    // Forward extended thinking (thinking/budget_tokens) if present
    if let Some(thinking) = req.get("thinking") {
        claude_req["thinking"] = thinking.clone();
    }

    // Forward tool_choice if present
    if let Some(tc) = req.get("tool_choice") {
        claude_req["tool_choice"] = convert_tool_choice(tc);
    }

    serde_json::to_vec(&claude_req).map_err(|e| ProxyError::Translation(e.to_string()))
}

fn extract_system_messages(req: &mut Value) -> String {
    let mut system_parts = Vec::new();
    if let Some(messages) = req.get("messages").and_then(|m| m.as_array()) {
        for msg in messages {
            if msg.get("role").and_then(|r| r.as_str()) == Some("system")
                && let Some(content) = msg.get("content")
            {
                match content {
                    Value::String(s) => system_parts.push(s.clone()),
                    Value::Array(parts) => {
                        for part in parts {
                            if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                                system_parts.push(text.to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    system_parts.join("\n\n")
}

fn convert_messages(req: &Value) -> Result<Vec<Value>, ProxyError> {
    let messages = req
        .get("messages")
        .and_then(|m| m.as_array())
        .ok_or_else(|| ProxyError::Translation("missing messages field".to_string()))?;

    let mut claude_messages: Vec<Value> = Vec::new();

    for msg in messages {
        let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");

        if role == "system" {
            continue;
        }

        if role == "tool" {
            // Convert tool result message to user message with tool_result content block
            let tool_call_id = msg
                .get("tool_call_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let content_text = match msg.get("content") {
                Some(Value::String(s)) => s.clone(),
                _ => String::new(),
            };

            let tool_result = json!({
                "type": "tool_result",
                "tool_use_id": tool_call_id,
                "content": content_text,
            });

            // Check if the last message is from the "user" role - merge tool results
            if let Some(last) = claude_messages.last_mut()
                && last.get("role").and_then(|r: &Value| r.as_str()) == Some("user")
                && let Some(content) = last.get_mut("content")
                && let Some(arr) = content.as_array_mut()
            {
                arr.push(tool_result);
                continue;
            }

            claude_messages.push(json!({
                "role": "user",
                "content": [tool_result],
            }));
            continue;
        }

        if role == "assistant" {
            let mut content_blocks = Vec::new();

            // Handle text content
            if let Some(content) = msg.get("content") {
                match content {
                    Value::String(s) if !s.is_empty() => {
                        content_blocks.push(json!({"type": "text", "text": s}));
                    }
                    _ => {}
                }
            }

            // Handle tool_calls -> tool_use blocks
            if let Some(tool_calls) = msg.get("tool_calls").and_then(|tc| tc.as_array()) {
                for tc in tool_calls {
                    let id = tc.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    let name = tc
                        .get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("");
                    let arguments_str = tc
                        .get("function")
                        .and_then(|f| f.get("arguments"))
                        .and_then(|a| a.as_str())
                        .unwrap_or("{}");
                    let input: Value = serde_json::from_str(arguments_str).unwrap_or_else(|e| {
                        tracing::debug!("Malformed tool_call arguments JSON: {e}");
                        json!({})
                    });

                    content_blocks.push(json!({
                        "type": "tool_use",
                        "id": id,
                        "name": name,
                        "input": input,
                    }));
                }
            }

            if content_blocks.is_empty() {
                content_blocks.push(json!({"type": "text", "text": ""}));
            }

            claude_messages.push(json!({
                "role": "assistant",
                "content": content_blocks,
            }));
            continue;
        }

        // User messages
        let claude_content = convert_user_content(msg.get("content"));
        claude_messages.push(json!({
            "role": "user",
            "content": claude_content,
        }));
    }

    Ok(claude_messages)
}

fn convert_user_content(content: Option<&Value>) -> Value {
    match content {
        Some(Value::String(s)) => Value::String(s.clone()),
        Some(Value::Array(parts)) => {
            let mut blocks = Vec::new();
            for part in parts {
                let part_type = part.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match part_type {
                    "text" => {
                        let text = part.get("text").and_then(|t| t.as_str()).unwrap_or("");
                        blocks.push(json!({"type": "text", "text": text}));
                    }
                    "image_url" => {
                        if let Some(url_obj) = part.get("image_url") {
                            let url = url_obj.get("url").and_then(|u| u.as_str()).unwrap_or("");
                            if let Some(image_block) = convert_image_url(url) {
                                blocks.push(image_block);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Value::Array(blocks)
        }
        _ => Value::String(String::new()),
    }
}

fn convert_image_url(url: &str) -> Option<Value> {
    // Handle base64 data URLs: data:image/png;base64,<data>
    if let Some(rest) = url.strip_prefix("data:") {
        let parts: Vec<&str> = rest.splitn(2, ',').collect();
        if parts.len() == 2 {
            let meta = parts[0]; // e.g., "image/png;base64"
            let data = parts[1];
            let media_type = meta.split(';').next().unwrap_or("image/png");
            return Some(json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": media_type,
                    "data": data,
                }
            }));
        }
    }
    // For regular URLs, use the url source type
    Some(json!({
        "type": "image",
        "source": {
            "type": "url",
            "url": url,
        }
    }))
}

fn convert_tools(req: &Value) -> Option<Value> {
    let tools = req.get("tools")?.as_array()?;
    let claude_tools: Vec<Value> = tools
        .iter()
        .filter_map(|tool| {
            let func = tool.get("function")?;
            let name = func.get("name")?.as_str()?;
            let description = func
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("");
            let parameters = func
                .get("parameters")
                .cloned()
                .unwrap_or(json!({"type": "object", "properties": {}}));
            Some(json!({
                "name": name,
                "description": description,
                "input_schema": parameters,
            }))
        })
        .collect();

    if claude_tools.is_empty() {
        None
    } else {
        Some(Value::Array(claude_tools))
    }
}

fn convert_stop_sequences(req: &Value) -> Option<Value> {
    let stop = req.get("stop")?;
    match stop {
        Value::String(s) => Some(json!([s])),
        Value::Array(_) => Some(stop.clone()),
        _ => None,
    }
}

fn convert_tool_choice(tc: &Value) -> Value {
    match tc {
        Value::String(s) => match s.as_str() {
            "none" => json!({"type": "none"}),
            "auto" => json!({"type": "auto"}),
            "required" => json!({"type": "any"}),
            _ => json!({"type": "auto"}),
        },
        Value::Object(obj) => {
            if let Some(func) = obj.get("function")
                && let Some(name) = func.get("name").and_then(|n| n.as_str())
            {
                return json!({"type": "tool", "name": name});
            }
            json!({"type": "auto"})
        }
        _ => json!({"type": "auto"}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_json_diff::assert_json_eq;

    fn translate(req: Value, stream: bool) -> Value {
        let raw = serde_json::to_vec(&req).unwrap();
        let result = translate_request("claude-3-5-sonnet-20241022", &raw, stream).unwrap();
        serde_json::from_slice(&result).unwrap()
    }

    // === Basic request translation ===

    #[test]
    fn test_basic_text_message() {
        let req = json!({
            "model": "gpt-4",
            "messages": [
                {"role": "user", "content": "Hello"}
            ]
        });
        let result = translate(req, false);
        assert_eq!(result["model"], "claude-3-5-sonnet-20241022");
        assert_eq!(result["messages"][0]["role"], "user");
        assert_eq!(result["messages"][0]["content"], "Hello");
        assert_eq!(result["max_tokens"], 8192);
        assert!(result.get("stream").is_none());
    }

    #[test]
    fn test_stream_flag_set() {
        let req = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hi"}]
        });
        let result = translate(req, true);
        assert_eq!(result["stream"], true);
    }

    // === System message extraction ===

    #[test]
    fn test_single_system_message() {
        let req = json!({
            "model": "gpt-4",
            "messages": [
                {"role": "system", "content": "You are helpful."},
                {"role": "user", "content": "Hi"}
            ]
        });
        let result = translate(req, false);
        assert_eq!(result["system"], "You are helpful.");
        // System message should not appear in messages
        assert_eq!(result["messages"].as_array().unwrap().len(), 1);
        assert_eq!(result["messages"][0]["role"], "user");
    }

    #[test]
    fn test_multiple_system_messages() {
        let req = json!({
            "model": "gpt-4",
            "messages": [
                {"role": "system", "content": "Rule 1"},
                {"role": "system", "content": "Rule 2"},
                {"role": "user", "content": "Hi"}
            ]
        });
        let result = translate(req, false);
        assert_eq!(result["system"], "Rule 1\n\nRule 2");
    }

    #[test]
    fn test_system_message_with_array_content() {
        let req = json!({
            "model": "gpt-4",
            "messages": [
                {"role": "system", "content": [
                    {"type": "text", "text": "Part A"},
                    {"type": "text", "text": "Part B"}
                ]},
                {"role": "user", "content": "Hi"}
            ]
        });
        let result = translate(req, false);
        assert_eq!(result["system"], "Part A\n\nPart B");
    }

    #[test]
    fn test_no_system_message() {
        let req = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hi"}]
        });
        let result = translate(req, false);
        assert!(result.get("system").is_none());
    }

    // === User content conversion ===

    #[test]
    fn test_user_multipart_content() {
        let req = json!({
            "model": "gpt-4",
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "text", "text": "Describe this:"},
                    {"type": "image_url", "image_url": {"url": "https://example.com/img.png"}}
                ]
            }]
        });
        let result = translate(req, false);
        let content = result["messages"][0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[1]["type"], "image");
        assert_eq!(content[1]["source"]["type"], "url");
    }

    #[test]
    fn test_user_base64_image() {
        let req = json!({
            "model": "gpt-4",
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "image_url", "image_url": {"url": "data:image/png;base64,iVBOR..."}}
                ]
            }]
        });
        let result = translate(req, false);
        let img = &result["messages"][0]["content"][0];
        assert_eq!(img["type"], "image");
        assert_eq!(img["source"]["type"], "base64");
        assert_eq!(img["source"]["media_type"], "image/png");
        assert_eq!(img["source"]["data"], "iVBOR...");
    }

    #[test]
    fn test_user_empty_content() {
        let req = json!({
            "model": "gpt-4",
            "messages": [{"role": "user"}]
        });
        let result = translate(req, false);
        assert_eq!(result["messages"][0]["content"], "");
    }

    // === Assistant message conversion ===

    #[test]
    fn test_assistant_text_message() {
        let req = json!({
            "model": "gpt-4",
            "messages": [
                {"role": "user", "content": "Hi"},
                {"role": "assistant", "content": "Hello!"},
                {"role": "user", "content": "Bye"}
            ]
        });
        let result = translate(req, false);
        let assistant = &result["messages"][1];
        assert_eq!(assistant["role"], "assistant");
        assert_eq!(assistant["content"][0]["type"], "text");
        assert_eq!(assistant["content"][0]["text"], "Hello!");
    }

    #[test]
    fn test_assistant_with_tool_calls() {
        let req = json!({
            "model": "gpt-4",
            "messages": [
                {"role": "user", "content": "Weather?"},
                {"role": "assistant", "content": null, "tool_calls": [{
                    "id": "call_123",
                    "type": "function",
                    "function": {"name": "get_weather", "arguments": "{\"city\":\"SF\"}"}
                }]}
            ]
        });
        let result = translate(req, false);
        let assistant = &result["messages"][1];
        let content = assistant["content"].as_array().unwrap();
        // Should have tool_use block (no text since content is null)
        let tool_use = content.iter().find(|b| b["type"] == "tool_use").unwrap();
        assert_eq!(tool_use["id"], "call_123");
        assert_eq!(tool_use["name"], "get_weather");
        assert_json_eq!(tool_use["input"], json!({"city": "SF"}));
    }

    #[test]
    fn test_assistant_empty_content_gets_placeholder() {
        let req = json!({
            "model": "gpt-4",
            "messages": [
                {"role": "user", "content": "Hi"},
                {"role": "assistant", "content": ""},
                {"role": "user", "content": "Bye"}
            ]
        });
        let result = translate(req, false);
        let blocks = result["messages"][1]["content"].as_array().unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["type"], "text");
        assert_eq!(blocks[0]["text"], "");
    }

    // === Tool result messages ===

    #[test]
    fn test_tool_result_message() {
        let req = json!({
            "model": "gpt-4",
            "messages": [
                {"role": "user", "content": "Weather?"},
                {"role": "assistant", "content": null, "tool_calls": [{
                    "id": "call_123",
                    "type": "function",
                    "function": {"name": "get_weather", "arguments": "{}"}
                }]},
                {"role": "tool", "tool_call_id": "call_123", "content": "72°F sunny"}
            ]
        });
        let result = translate(req, false);
        // Tool result should be a user message with tool_result content block
        let tool_msg = &result["messages"][2];
        assert_eq!(tool_msg["role"], "user");
        let content = tool_msg["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "tool_result");
        assert_eq!(content[0]["tool_use_id"], "call_123");
        assert_eq!(content[0]["content"], "72°F sunny");
    }

    #[test]
    fn test_multiple_tool_results_merge() {
        let req = json!({
            "model": "gpt-4",
            "messages": [
                {"role": "user", "content": "Weather?"},
                {"role": "assistant", "content": null, "tool_calls": [
                    {"id": "call_1", "type": "function", "function": {"name": "weather", "arguments": "{}"}},
                    {"id": "call_2", "type": "function", "function": {"name": "time", "arguments": "{}"}}
                ]},
                {"role": "tool", "tool_call_id": "call_1", "content": "72°F"},
                {"role": "tool", "tool_call_id": "call_2", "content": "3:00 PM"}
            ]
        });
        let result = translate(req, false);
        // Both tool results should be merged into a single user message
        let msgs = result["messages"].as_array().unwrap();
        // user, assistant, user(tool_results merged)
        assert_eq!(msgs.len(), 3);
        let tool_msg = &msgs[2];
        assert_eq!(tool_msg["role"], "user");
        let content = tool_msg["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);
        assert_eq!(content[0]["tool_use_id"], "call_1");
        assert_eq!(content[1]["tool_use_id"], "call_2");
    }

    // === Tools conversion ===

    #[test]
    fn test_tools_conversion() {
        let req = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hi"}],
            "tools": [{
                "type": "function",
                "function": {
                    "name": "get_weather",
                    "description": "Get weather for a city",
                    "parameters": {
                        "type": "object",
                        "properties": {"city": {"type": "string"}},
                        "required": ["city"]
                    }
                }
            }]
        });
        let result = translate(req, false);
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "get_weather");
        assert_eq!(tools[0]["description"], "Get weather for a city");
        assert!(tools[0]["input_schema"]["properties"]["city"].is_object());
    }

    #[test]
    fn test_no_tools() {
        let req = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hi"}]
        });
        let result = translate(req, false);
        assert!(result.get("tools").is_none());
    }

    // === Tool choice conversion ===

    #[test]
    fn test_tool_choice_none() {
        let req = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hi"}],
            "tool_choice": "none"
        });
        let result = translate(req, false);
        assert_json_eq!(result["tool_choice"], json!({"type": "none"}));
    }

    #[test]
    fn test_tool_choice_auto() {
        let req = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hi"}],
            "tool_choice": "auto"
        });
        let result = translate(req, false);
        assert_json_eq!(result["tool_choice"], json!({"type": "auto"}));
    }

    #[test]
    fn test_tool_choice_required() {
        let req = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hi"}],
            "tool_choice": "required"
        });
        let result = translate(req, false);
        assert_json_eq!(result["tool_choice"], json!({"type": "any"}));
    }

    #[test]
    fn test_tool_choice_specific_function() {
        let req = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hi"}],
            "tool_choice": {"type": "function", "function": {"name": "get_weather"}}
        });
        let result = translate(req, false);
        assert_json_eq!(
            result["tool_choice"],
            json!({"type": "tool", "name": "get_weather"})
        );
    }

    // === Stop sequences ===

    #[test]
    fn test_stop_string() {
        let req = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hi"}],
            "stop": "END"
        });
        let result = translate(req, false);
        assert_json_eq!(result["stop_sequences"], json!(["END"]));
    }

    #[test]
    fn test_stop_array() {
        let req = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hi"}],
            "stop": ["END", "STOP"]
        });
        let result = translate(req, false);
        assert_json_eq!(result["stop_sequences"], json!(["END", "STOP"]));
    }

    // === Parameter passthrough ===

    #[test]
    fn test_temperature_passthrough() {
        let req = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hi"}],
            "temperature": 0.7,
            "top_p": 0.9
        });
        let result = translate(req, false);
        assert_eq!(result["temperature"], 0.7);
        assert_eq!(result["top_p"], 0.9);
    }

    #[test]
    fn test_max_completion_tokens_fallback() {
        let req = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hi"}],
            "max_completion_tokens": 4096
        });
        let result = translate(req, false);
        assert_eq!(result["max_tokens"], 4096);
    }

    #[test]
    fn test_max_tokens_takes_precedence() {
        let req = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hi"}],
            "max_tokens": 1024,
            "max_completion_tokens": 4096
        });
        let result = translate(req, false);
        assert_eq!(result["max_tokens"], 1024);
    }

    #[test]
    fn test_extended_thinking_passthrough() {
        let req = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hi"}],
            "thinking": {"type": "enabled", "budget_tokens": 10000}
        });
        let result = translate(req, false);
        assert_json_eq!(
            result["thinking"],
            json!({"type": "enabled", "budget_tokens": 10000})
        );
    }

    // === Error handling ===

    #[test]
    fn test_missing_messages_field() {
        let req = json!({"model": "gpt-4"});
        let raw = serde_json::to_vec(&req).unwrap();
        let result = translate_request("claude-3-5-sonnet-20241022", &raw, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_json() {
        let raw = b"not valid json";
        let result = translate_request("claude-3-5-sonnet-20241022", raw, false);
        assert!(result.is_err());
    }
}
