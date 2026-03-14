use prism_types::error::ProxyError;
use serde_json::{Value, json};

pub fn translate_request(
    model: &str,
    raw_json: &[u8],
    stream: bool,
) -> Result<Vec<u8>, ProxyError> {
    let req: Value = serde_json::from_slice(raw_json)?;

    // 1. Convert contents → messages
    let mut messages = convert_contents(&req)?;

    // 2. Extract system instruction → system message (prepend)
    if let Some(system_msg) = extract_system_message(&req) {
        messages.insert(0, system_msg);
    }

    // 3. Build OpenAI request
    let mut openai_req = json!({
        "model": model,
        "messages": messages,
    });

    if stream {
        openai_req["stream"] = Value::Bool(true);
    }

    // 4. Map generationConfig → top-level params
    if let Some(gc) = req.get("generationConfig") {
        if let Some(temp) = gc.get("temperature") {
            openai_req["temperature"] = temp.clone();
        }
        if let Some(top_p) = gc.get("topP") {
            openai_req["top_p"] = top_p.clone();
        }
        if let Some(max) = gc.get("maxOutputTokens") {
            openai_req["max_tokens"] = max.clone();
        }
        if let Some(stop) = gc.get("stopSequences") {
            openai_req["stop"] = stop.clone();
        }
    }

    // 5. Convert tools
    if let Some(tools) = convert_tools(&req) {
        openai_req["tools"] = tools;
    }

    serde_json::to_vec(&openai_req).map_err(|e| ProxyError::Translation(e.to_string()))
}

fn extract_system_message(req: &Value) -> Option<Value> {
    let si = req.get("systemInstruction")?;
    let parts = si.get("parts")?.as_array()?;

    let mut text_parts = Vec::new();
    for part in parts {
        if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
            text_parts.push(text.to_string());
        }
    }

    if text_parts.is_empty() {
        None
    } else {
        Some(json!({
            "role": "system",
            "content": text_parts.join("\n\n"),
        }))
    }
}

fn convert_contents(req: &Value) -> Result<Vec<Value>, ProxyError> {
    let contents = req
        .get("contents")
        .and_then(|c| c.as_array())
        .ok_or_else(|| ProxyError::Translation("missing contents field".to_string()))?;

    let mut messages = Vec::new();
    // Track functionCall name→id for matching with functionResponse
    let mut pending_tool_ids: Vec<(String, String)> = Vec::new();

    for content in contents {
        let role = content
            .get("role")
            .and_then(|r| r.as_str())
            .unwrap_or("user");
        let parts = content.get("parts").and_then(|p| p.as_array());

        let openai_role = match role {
            "model" => "assistant",
            _ => "user",
        };

        if let Some(parts) = parts {
            let mut text_parts = Vec::new();
            let mut tool_calls = Vec::new();
            let mut function_responses = Vec::new();
            let mut tc_index = 0u32;

            for part in parts {
                if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                    text_parts.push(text.to_string());
                } else if let Some(fc) = part.get("functionCall") {
                    let name = fc.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    let args = fc.get("args").cloned().unwrap_or(json!({}));
                    let arguments = serde_json::to_string(&args).unwrap_or_default();
                    let tc_id = format!("call_{}", uuid::Uuid::new_v4());

                    pending_tool_ids.push((name.to_string(), tc_id.clone()));

                    tool_calls.push(json!({
                        "id": tc_id,
                        "type": "function",
                        "function": {
                            "name": name,
                            "arguments": arguments,
                        },
                        "index": tc_index,
                    }));
                    tc_index += 1;
                } else if let Some(fr) = part.get("functionResponse") {
                    let name = fr
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("function");
                    let response = fr.get("response").cloned().unwrap_or(json!({}));
                    let content_str = serde_json::to_string(&response).unwrap_or_default();

                    // Match to pending tool_call_id by name
                    let tool_call_id =
                        if let Some(pos) = pending_tool_ids.iter().position(|(n, _)| n == name) {
                            let (_, id) = pending_tool_ids.remove(pos);
                            id
                        } else {
                            format!("call_{}", uuid::Uuid::new_v4())
                        };

                    function_responses.push(json!({
                        "role": "tool",
                        "tool_call_id": tool_call_id,
                        "name": name,
                        "content": content_str,
                    }));
                }
            }

            // Emit messages based on what was found
            if !function_responses.is_empty() {
                if !text_parts.is_empty() {
                    messages.push(json!({"role": "user", "content": text_parts.join("")}));
                }
                for fr in function_responses {
                    messages.push(fr);
                }
            } else if !tool_calls.is_empty() {
                let content = if text_parts.is_empty() {
                    Value::Null
                } else {
                    Value::String(text_parts.join(""))
                };
                messages.push(json!({
                    "role": "assistant",
                    "content": content,
                    "tool_calls": tool_calls,
                }));
            } else {
                messages.push(json!({
                    "role": openai_role,
                    "content": text_parts.join(""),
                }));
            }
        } else {
            messages.push(json!({
                "role": openai_role,
                "content": "",
            }));
        }
    }

    Ok(messages)
}

fn convert_tools(req: &Value) -> Option<Value> {
    let tools = req.get("tools")?.as_array()?;
    let mut openai_tools = Vec::new();

    for tool in tools {
        if let Some(decls) = tool.get("functionDeclarations").and_then(|d| d.as_array()) {
            for decl in decls {
                let name = decl.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let description = decl
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("");
                let parameters = decl.get("parameters").cloned();

                let mut func = json!({
                    "name": name,
                    "description": description,
                });
                if let Some(params) = parameters {
                    func["parameters"] = params;
                }

                openai_tools.push(json!({
                    "type": "function",
                    "function": func,
                }));
            }
        }
    }

    if openai_tools.is_empty() {
        None
    } else {
        Some(Value::Array(openai_tools))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn translate(req: Value) -> Value {
        let raw = serde_json::to_vec(&req).unwrap();
        let result = translate_request("gpt-4", &raw, false).unwrap();
        serde_json::from_slice(&result).unwrap()
    }

    #[test]
    fn test_basic_text_message() {
        let req = json!({
            "contents": [{"role": "user", "parts": [{"text": "Hello"}]}]
        });
        let result = translate(req);
        assert_eq!(result["model"], "gpt-4");
        assert_eq!(result["messages"][0]["role"], "user");
        assert_eq!(result["messages"][0]["content"], "Hello");
    }

    #[test]
    fn test_system_instruction() {
        let req = json!({
            "systemInstruction": {"parts": [{"text": "Be helpful"}]},
            "contents": [{"role": "user", "parts": [{"text": "Hi"}]}]
        });
        let result = translate(req);
        assert_eq!(result["messages"][0]["role"], "system");
        assert_eq!(result["messages"][0]["content"], "Be helpful");
        assert_eq!(result["messages"][1]["role"], "user");
        assert_eq!(result["messages"][1]["content"], "Hi");
    }

    #[test]
    fn test_system_instruction_multiple_parts() {
        let req = json!({
            "systemInstruction": {"parts": [{"text": "Rule 1"}, {"text": "Rule 2"}]},
            "contents": [{"role": "user", "parts": [{"text": "Hi"}]}]
        });
        let result = translate(req);
        assert_eq!(result["messages"][0]["content"], "Rule 1\n\nRule 2");
    }

    #[test]
    fn test_model_role_to_assistant() {
        let req = json!({
            "contents": [
                {"role": "user", "parts": [{"text": "Hi"}]},
                {"role": "model", "parts": [{"text": "Hello!"}]},
                {"role": "user", "parts": [{"text": "Bye"}]}
            ]
        });
        let result = translate(req);
        assert_eq!(result["messages"][0]["role"], "user");
        assert_eq!(result["messages"][1]["role"], "assistant");
        assert_eq!(result["messages"][1]["content"], "Hello!");
        assert_eq!(result["messages"][2]["role"], "user");
    }

    #[test]
    fn test_function_call_to_tool_calls() {
        let req = json!({
            "contents": [{
                "role": "model",
                "parts": [{
                    "functionCall": {
                        "name": "get_weather",
                        "args": {"city": "SF"}
                    }
                }]
            }]
        });
        let result = translate(req);
        assert_eq!(result["messages"][0]["role"], "assistant");
        assert_eq!(result["messages"][0]["content"], Value::Null);
        let tc = &result["messages"][0]["tool_calls"][0];
        assert_eq!(tc["type"], "function");
        assert_eq!(tc["function"]["name"], "get_weather");
        let args: Value =
            serde_json::from_str(tc["function"]["arguments"].as_str().unwrap()).unwrap();
        assert_eq!(args, json!({"city": "SF"}));
    }

    #[test]
    fn test_function_call_with_text() {
        let req = json!({
            "contents": [{
                "role": "model",
                "parts": [
                    {"text": "Let me check."},
                    {"functionCall": {"name": "search", "args": {"q": "test"}}}
                ]
            }]
        });
        let result = translate(req);
        assert_eq!(result["messages"][0]["role"], "assistant");
        assert_eq!(result["messages"][0]["content"], "Let me check.");
        assert_eq!(
            result["messages"][0]["tool_calls"][0]["function"]["name"],
            "search"
        );
    }

    #[test]
    fn test_function_response_to_tool_message() {
        let req = json!({
            "contents": [
                {"role": "user", "parts": [{"text": "Weather?"}]},
                {"role": "model", "parts": [{"functionCall": {"name": "weather", "args": {}}}]},
                {"role": "user", "parts": [{"functionResponse": {"name": "weather", "response": {"temp": 72}}}]}
            ]
        });
        let result = translate(req);
        let tool_msg = &result["messages"][2];
        assert_eq!(tool_msg["role"], "tool");
        assert_eq!(tool_msg["name"], "weather");
        // tool_call_id should match the one generated for the functionCall
        let tc_id = result["messages"][1]["tool_calls"][0]["id"]
            .as_str()
            .unwrap();
        assert_eq!(tool_msg["tool_call_id"], tc_id);
        let content: Value = serde_json::from_str(tool_msg["content"].as_str().unwrap()).unwrap();
        assert_eq!(content, json!({"temp": 72}));
    }

    #[test]
    fn test_generation_config_mapped() {
        let req = json!({
            "contents": [{"role": "user", "parts": [{"text": "Hi"}]}],
            "generationConfig": {
                "temperature": 0.7,
                "topP": 0.9,
                "maxOutputTokens": 1024,
                "stopSequences": ["END"]
            }
        });
        let result = translate(req);
        assert_eq!(result["temperature"], 0.7);
        assert_eq!(result["top_p"], 0.9);
        assert_eq!(result["max_tokens"], 1024);
        assert_eq!(result["stop"][0], "END");
    }

    #[test]
    fn test_no_generation_config() {
        let req = json!({
            "contents": [{"role": "user", "parts": [{"text": "Hi"}]}]
        });
        let result = translate(req);
        assert!(result.get("temperature").is_none());
        assert!(result.get("max_tokens").is_none());
    }

    #[test]
    fn test_tools_converted() {
        let req = json!({
            "contents": [{"role": "user", "parts": [{"text": "Hi"}]}],
            "tools": [{
                "functionDeclarations": [{
                    "name": "search",
                    "description": "Search the web",
                    "parameters": {"type": "object", "properties": {"q": {"type": "string"}}}
                }]
            }]
        });
        let result = translate(req);
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["type"], "function");
        assert_eq!(tools[0]["function"]["name"], "search");
        assert_eq!(tools[0]["function"]["description"], "Search the web");
        assert!(tools[0]["function"]["parameters"]["properties"]["q"].is_object());
    }

    #[test]
    fn test_stream_flag() {
        let req = json!({
            "contents": [{"role": "user", "parts": [{"text": "Hi"}]}]
        });
        let raw = serde_json::to_vec(&req).unwrap();
        let result: Value =
            serde_json::from_slice(&translate_request("gpt-4", &raw, true).unwrap()).unwrap();
        assert_eq!(result["stream"], true);
    }

    #[test]
    fn test_no_stream_flag_when_false() {
        let req = json!({
            "contents": [{"role": "user", "parts": [{"text": "Hi"}]}]
        });
        let result = translate(req);
        assert!(result.get("stream").is_none());
    }

    #[test]
    fn test_missing_contents_error() {
        let req = json!({});
        let raw = serde_json::to_vec(&req).unwrap();
        let result = translate_request("gpt-4", &raw, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_no_system_instruction() {
        let req = json!({
            "contents": [{"role": "user", "parts": [{"text": "Hi"}]}]
        });
        let result = translate(req);
        // Should have only the user message, no system
        assert_eq!(result["messages"].as_array().unwrap().len(), 1);
        assert_eq!(result["messages"][0]["role"], "user");
    }
}
