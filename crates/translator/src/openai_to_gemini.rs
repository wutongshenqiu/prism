use prism_types::error::ProxyError;
use serde_json::{Value, json};

pub fn translate_request(
    model: &str,
    raw_json: &[u8],
    _stream: bool,
) -> Result<Vec<u8>, ProxyError> {
    let req: Value = serde_json::from_slice(raw_json)?;

    // 1. Extract system messages -> systemInstruction
    let system_instruction = extract_system_instruction(&req);

    // 2. Convert messages -> contents
    let contents = convert_messages(&req)?;

    // 3. Convert tools
    let tools = convert_tools(&req);

    // 4. Build generationConfig
    let generation_config = build_generation_config(&req);

    // Build Gemini request
    let mut gemini_req = json!({
        "contents": contents,
    });

    if let Some(si) = system_instruction {
        gemini_req["systemInstruction"] = si;
    }
    if let Some(gc) = generation_config {
        gemini_req["generationConfig"] = gc;
    }
    if let Some(tools) = tools {
        gemini_req["tools"] = tools;
    }

    // model is used in URL routing, not in the body for Gemini
    let _ = model;

    serde_json::to_vec(&gemini_req).map_err(|e| ProxyError::Translation(e.to_string()))
}

fn extract_system_instruction(req: &Value) -> Option<Value> {
    let messages = req.get("messages")?.as_array()?;
    let mut system_parts = Vec::new();

    for msg in messages {
        if msg.get("role").and_then(|r| r.as_str()) == Some("system")
            && let Some(content) = msg.get("content")
        {
            match content {
                Value::String(s) => {
                    system_parts.push(json!({"text": s}));
                }
                Value::Array(parts) => {
                    for part in parts {
                        if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                            system_parts.push(json!({"text": text}));
                        }
                    }
                }
                _ => {}
            }
        }
    }

    if system_parts.is_empty() {
        None
    } else {
        Some(json!({
            "parts": system_parts,
        }))
    }
}

fn convert_messages(req: &Value) -> Result<Vec<Value>, ProxyError> {
    let messages = req
        .get("messages")
        .and_then(|m| m.as_array())
        .ok_or_else(|| ProxyError::Translation("missing messages field".to_string()))?;

    let mut contents: Vec<Value> = Vec::new();

    for msg in messages {
        let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");

        if role == "system" {
            continue;
        }

        if role == "tool" {
            // Convert to functionResponse part
            let name = msg
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("function");
            let content_text = msg.get("content").and_then(|c| c.as_str()).unwrap_or("");

            // Try to parse content as JSON, fallback to wrapping in result object
            let response_val = serde_json::from_str::<Value>(content_text)
                .unwrap_or_else(|_| json!({"result": content_text}));

            let part = json!({
                "functionResponse": {
                    "name": name,
                    "response": response_val,
                }
            });

            // Merge with previous user content if last was also user/function
            if let Some(last) = contents.last_mut()
                && last.get("role").and_then(|r: &Value| r.as_str()) == Some("user")
                && let Some(parts) = last
                    .get_mut("parts")
                    .and_then(|p: &mut Value| p.as_array_mut())
            {
                parts.push(part);
                continue;
            }

            contents.push(json!({
                "role": "user",
                "parts": [part],
            }));
            continue;
        }

        let gemini_role = match role {
            "assistant" => "model",
            _ => "user",
        };

        let parts = convert_content_to_parts(msg)?;

        // If the role matches the previous message, merge parts
        if let Some(last) = contents.last_mut()
            && last.get("role").and_then(|r: &Value| r.as_str()) == Some(gemini_role)
            && let Some(existing_parts) = last
                .get_mut("parts")
                .and_then(|p: &mut Value| p.as_array_mut())
        {
            existing_parts.extend(parts);
            continue;
        }

        contents.push(json!({
            "role": gemini_role,
            "parts": parts,
        }));
    }

    Ok(contents)
}

fn convert_content_to_parts(msg: &Value) -> Result<Vec<Value>, ProxyError> {
    let mut parts = Vec::new();

    // Handle text/multipart content
    if let Some(content) = msg.get("content") {
        match content {
            Value::String(s) => {
                parts.push(json!({"text": s}));
            }
            Value::Array(content_parts) => {
                for part in content_parts {
                    let part_type = part.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    match part_type {
                        "text" => {
                            let text = part.get("text").and_then(|t| t.as_str()).unwrap_or("");
                            parts.push(json!({"text": text}));
                        }
                        "image_url" => {
                            if let Some(url_obj) = part.get("image_url") {
                                let url = url_obj.get("url").and_then(|u| u.as_str()).unwrap_or("");
                                if let Some(inline) = convert_image_url_to_inline(url) {
                                    parts.push(inline);
                                }
                            }
                        }
                        "file" => {
                            if let Some(file_obj) = part.get("file") {
                                let url =
                                    file_obj.get("url").and_then(|u| u.as_str()).unwrap_or("");
                                if let Some(rest) = url.strip_prefix("data:") {
                                    let segs: Vec<&str> = rest.splitn(2, ',').collect();
                                    if segs.len() == 2 {
                                        let meta = segs[0];
                                        let data = segs[1];
                                        let mime_type = meta
                                            .split(';')
                                            .next()
                                            .unwrap_or("application/octet-stream");
                                        parts.push(json!({
                                            "inlineData": {
                                                "mimeType": mime_type,
                                                "data": data,
                                            }
                                        }));
                                    }
                                } else if url.starts_with("http://") || url.starts_with("https://")
                                {
                                    let mime_type = infer_mime_type_from_url(url);
                                    parts.push(json!({
                                        "fileData": {
                                            "mimeType": mime_type,
                                            "fileUri": url,
                                        }
                                    }));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            Value::Null => {}
            _ => {}
        }
    }

    // Handle tool_calls in assistant messages -> functionCall parts
    if let Some(tool_calls) = msg.get("tool_calls").and_then(|tc| tc.as_array()) {
        for tc in tool_calls {
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
            let args: Value = serde_json::from_str(arguments_str).unwrap_or_else(|e| {
                tracing::debug!("Malformed tool_call arguments JSON: {e}");
                json!({})
            });

            parts.push(json!({
                "functionCall": {
                    "name": name,
                    "args": args,
                }
            }));
        }
    }

    if parts.is_empty() {
        parts.push(json!({"text": ""}));
    }

    Ok(parts)
}

fn convert_image_url_to_inline(url: &str) -> Option<Value> {
    if let Some(rest) = url.strip_prefix("data:") {
        let parts: Vec<&str> = rest.splitn(2, ',').collect();
        if parts.len() == 2 {
            let meta = parts[0];
            let data = parts[1];
            let mime_type = meta.split(';').next().unwrap_or("image/png");
            return Some(json!({
                "inlineData": {
                    "mimeType": mime_type,
                    "data": data,
                }
            }));
        }
    }
    // For remote URLs, use Gemini's fileData format
    if url.starts_with("http://") || url.starts_with("https://") {
        let mime_type = infer_mime_type_from_url(url);
        return Some(json!({
            "fileData": {
                "mimeType": mime_type,
                "fileUri": url,
            }
        }));
    }
    // For other schemes, fall back to text reference
    Some(json!({"text": format!("[image: {}]", url)}))
}

fn infer_mime_type_from_url(url: &str) -> &str {
    let path = url.split('?').next().unwrap_or(url);
    if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".gif") {
        "image/gif"
    } else if path.ends_with(".webp") {
        "image/webp"
    } else if path.ends_with(".pdf") {
        "application/pdf"
    } else {
        "image/jpeg"
    }
}

fn convert_tools(req: &Value) -> Option<Value> {
    let tools = req.get("tools")?.as_array()?;
    let mut function_declarations = Vec::new();

    for tool in tools {
        if let Some(func) = tool.get("function") {
            let name = func.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let description = func
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("");
            let parameters = func.get("parameters").cloned();

            let mut decl = json!({
                "name": name,
                "description": description,
            });
            if let Some(params) = parameters {
                decl["parameters"] = params;
            }

            function_declarations.push(decl);
        }
    }

    if function_declarations.is_empty() {
        None
    } else {
        Some(json!([{
            "functionDeclarations": function_declarations,
        }]))
    }
}

fn build_generation_config(req: &Value) -> Option<Value> {
    let mut config = json!({});
    let mut has_any = false;

    if let Some(temp) = req.get("temperature") {
        config["temperature"] = temp.clone();
        has_any = true;
    }
    if let Some(top_p) = req.get("top_p") {
        config["topP"] = top_p.clone();
        has_any = true;
    }
    if let Some(max) = req.get("max_tokens").or(req.get("max_completion_tokens")) {
        config["maxOutputTokens"] = max.clone();
        has_any = true;
    }
    if let Some(stop) = req.get("stop") {
        match stop {
            Value::String(s) => {
                config["stopSequences"] = json!([s]);
                has_any = true;
            }
            Value::Array(_) => {
                config["stopSequences"] = stop.clone();
                has_any = true;
            }
            _ => {}
        }
    }

    // Map reasoning_effort → thinkingConfig.thinkingBudget
    if let Some(effort) = req.get("reasoning_effort").and_then(|e| e.as_str()) {
        let max_tokens = req
            .get("max_tokens")
            .or(req.get("max_completion_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(8192);
        let budget = match effort {
            "low" => 1024u64,
            "medium" => 4096,
            "high" => (max_tokens.max(8192) as f64 * 0.8) as u64,
            _ => 0,
        };
        if budget > 0 {
            config["thinkingConfig"] = json!({
                "thinkingBudget": budget,
            });
            has_any = true;
        }
    }

    // Forward explicit thinking config (from model suffix parsing)
    if let Some(thinking) = req.get("thinking")
        && let Some(budget) = thinking.get("budget_tokens").and_then(|b| b.as_u64())
    {
        config["thinkingConfig"] = json!({
            "thinkingBudget": budget,
        });
        has_any = true;
    }

    // Handle response_format translation
    if let Some(rf) = req.get("response_format") {
        let rf_type = rf.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match rf_type {
            "json_schema" => {
                if let Some(schema_obj) = rf.get("json_schema") {
                    config["responseMimeType"] = json!("application/json");
                    if let Some(schema) = schema_obj.get("schema") {
                        config["responseSchema"] = schema.clone();
                    }
                    has_any = true;
                }
            }
            "json_object" => {
                config["responseMimeType"] = json!("application/json");
                has_any = true;
            }
            _ => {}
        }
    }

    if has_any { Some(config) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_json_diff::assert_json_eq;

    fn translate(req: Value) -> Value {
        let raw = serde_json::to_vec(&req).unwrap();
        let result = translate_request("gemini-1.5-pro", &raw, false).unwrap();
        serde_json::from_slice(&result).unwrap()
    }

    #[test]
    fn test_basic_text_message() {
        let req = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello"}]
        });
        let result = translate(req);
        assert_eq!(result["contents"][0]["role"], "user");
        assert_eq!(result["contents"][0]["parts"][0]["text"], "Hello");
    }

    #[test]
    fn test_system_instruction_extraction() {
        let req = json!({
            "model": "gpt-4",
            "messages": [
                {"role": "system", "content": "You are helpful."},
                {"role": "user", "content": "Hi"}
            ]
        });
        let result = translate(req);
        assert_eq!(
            result["systemInstruction"]["parts"][0]["text"],
            "You are helpful."
        );
        // System should be filtered from contents
        assert_eq!(result["contents"].as_array().unwrap().len(), 1);
        assert_eq!(result["contents"][0]["role"], "user");
    }

    #[test]
    fn test_system_instruction_array_content() {
        let req = json!({
            "model": "gpt-4",
            "messages": [
                {"role": "system", "content": [
                    {"type": "text", "text": "Rule 1"},
                    {"type": "text", "text": "Rule 2"}
                ]},
                {"role": "user", "content": "Hi"}
            ]
        });
        let result = translate(req);
        let parts = result["systemInstruction"]["parts"].as_array().unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0]["text"], "Rule 1");
        assert_eq!(parts[1]["text"], "Rule 2");
    }

    #[test]
    fn test_no_system_instruction() {
        let req = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hi"}]
        });
        let result = translate(req);
        assert!(result.get("systemInstruction").is_none());
    }

    #[test]
    fn test_assistant_role_mapped_to_model() {
        let req = json!({
            "model": "gpt-4",
            "messages": [
                {"role": "user", "content": "Hi"},
                {"role": "assistant", "content": "Hello!"},
                {"role": "user", "content": "Bye"}
            ]
        });
        let result = translate(req);
        assert_eq!(result["contents"][1]["role"], "model");
        assert_eq!(result["contents"][1]["parts"][0]["text"], "Hello!");
    }

    #[test]
    fn test_consecutive_same_role_merged() {
        let req = json!({
            "model": "gpt-4",
            "messages": [
                {"role": "user", "content": "Part 1"},
                {"role": "user", "content": "Part 2"}
            ]
        });
        let result = translate(req);
        // Gemini requires consecutive same-role messages to be merged
        let contents = result["contents"].as_array().unwrap();
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0]["parts"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_tool_calls_to_function_call() {
        let req = json!({
            "model": "gpt-4",
            "messages": [{
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "get_weather",
                        "arguments": "{\"city\":\"SF\"}"
                    }
                }]
            }]
        });
        let result = translate(req);
        let parts = &result["contents"][0]["parts"];
        let fc = &parts[0]["functionCall"];
        assert_eq!(fc["name"], "get_weather");
        assert_json_eq!(fc["args"], json!({"city": "SF"}));
    }

    #[test]
    fn test_tool_result_to_function_response() {
        let req = json!({
            "model": "gpt-4",
            "messages": [
                {"role": "user", "content": "Weather?"},
                {"role": "assistant", "content": null, "tool_calls": [{
                    "id": "call_1", "type": "function",
                    "function": {"name": "weather", "arguments": "{}"}
                }]},
                {"role": "tool", "name": "weather", "content": "{\"temp\": 72}"}
            ]
        });
        let result = translate(req);
        let tool_msg = result["contents"].as_array().unwrap().last().unwrap();
        assert_eq!(tool_msg["role"], "user");
        let fr = &tool_msg["parts"][0]["functionResponse"];
        assert_eq!(fr["name"], "weather");
        assert_json_eq!(fr["response"], json!({"temp": 72}));
    }

    #[test]
    fn test_tool_result_non_json_content() {
        let req = json!({
            "model": "gpt-4",
            "messages": [
                {"role": "tool", "name": "search", "content": "plain text result"}
            ]
        });
        let result = translate(req);
        let fr = &result["contents"][0]["parts"][0]["functionResponse"];
        assert_eq!(fr["name"], "search");
        assert_json_eq!(fr["response"], json!({"result": "plain text result"}));
    }

    #[test]
    fn test_base64_image_to_inline_data() {
        let req = json!({
            "model": "gpt-4",
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "image_url", "image_url": {"url": "data:image/jpeg;base64,/9j/4AAQ..."}}
                ]
            }]
        });
        let result = translate(req);
        let part = &result["contents"][0]["parts"][0];
        assert_eq!(part["inlineData"]["mimeType"], "image/jpeg");
        assert_eq!(part["inlineData"]["data"], "/9j/4AAQ...");
    }

    #[test]
    fn test_remote_image_url_to_file_data() {
        let req = json!({
            "model": "gpt-4",
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "image_url", "image_url": {"url": "https://example.com/image.png"}}
                ]
            }]
        });
        let result = translate(req);
        let part = &result["contents"][0]["parts"][0];
        assert_eq!(part["fileData"]["mimeType"], "image/png");
        assert_eq!(part["fileData"]["fileUri"], "https://example.com/image.png");
    }

    #[test]
    fn test_remote_image_url_mime_inference() {
        // .gif extension
        let part = convert_image_url_to_inline("https://example.com/anim.gif").unwrap();
        assert_eq!(part["fileData"]["mimeType"], "image/gif");

        // .webp extension
        let part = convert_image_url_to_inline("https://example.com/photo.webp").unwrap();
        assert_eq!(part["fileData"]["mimeType"], "image/webp");

        // no extension defaults to image/jpeg
        let part = convert_image_url_to_inline("https://example.com/photo?id=123").unwrap();
        assert_eq!(part["fileData"]["mimeType"], "image/jpeg");

        // .pdf extension
        let part = convert_image_url_to_inline("https://example.com/doc.pdf").unwrap();
        assert_eq!(part["fileData"]["mimeType"], "application/pdf");
    }

    #[test]
    fn test_file_content_part_base64_pdf() {
        let req = json!({
            "model": "gpt-4",
            "messages": [{
                "role": "user",
                "content": [
                    {
                        "type": "file",
                        "file": {
                            "url": "data:application/pdf;base64,JVBERi0xLjQ="
                        }
                    }
                ]
            }]
        });
        let result = translate(req);
        let part = &result["contents"][0]["parts"][0];
        assert_eq!(part["inlineData"]["mimeType"], "application/pdf");
        assert_eq!(part["inlineData"]["data"], "JVBERi0xLjQ=");
    }

    #[test]
    fn test_file_content_part_remote_url() {
        let req = json!({
            "model": "gpt-4",
            "messages": [{
                "role": "user",
                "content": [
                    {
                        "type": "file",
                        "file": {
                            "url": "https://example.com/report.pdf"
                        }
                    }
                ]
            }]
        });
        let result = translate(req);
        let part = &result["contents"][0]["parts"][0];
        assert_eq!(part["fileData"]["mimeType"], "application/pdf");
        assert_eq!(
            part["fileData"]["fileUri"],
            "https://example.com/report.pdf"
        );
    }

    #[test]
    fn test_tools_to_function_declarations() {
        let req = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hi"}],
            "tools": [{
                "type": "function",
                "function": {
                    "name": "search",
                    "description": "Search the web",
                    "parameters": {"type": "object", "properties": {"q": {"type": "string"}}}
                }
            }]
        });
        let result = translate(req);
        let decls = &result["tools"][0]["functionDeclarations"];
        assert_eq!(decls[0]["name"], "search");
        assert_eq!(decls[0]["description"], "Search the web");
        assert!(decls[0]["parameters"]["properties"]["q"].is_object());
    }

    #[test]
    fn test_generation_config() {
        let req = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hi"}],
            "temperature": 0.5,
            "top_p": 0.8,
            "max_tokens": 2048,
            "stop": ["END", "STOP"]
        });
        let result = translate(req);
        let gc = &result["generationConfig"];
        assert_eq!(gc["temperature"], 0.5);
        assert_eq!(gc["topP"], 0.8);
        assert_eq!(gc["maxOutputTokens"], 2048);
        assert_json_eq!(gc["stopSequences"], json!(["END", "STOP"]));
    }

    #[test]
    fn test_generation_config_stop_string() {
        let req = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hi"}],
            "stop": "END"
        });
        let result = translate(req);
        assert_json_eq!(result["generationConfig"]["stopSequences"], json!(["END"]));
    }

    #[test]
    fn test_no_generation_config_when_empty() {
        let req = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hi"}]
        });
        let result = translate(req);
        assert!(result.get("generationConfig").is_none());
    }

    #[test]
    fn test_reasoning_effort_low_to_thinking_config() {
        let req = json!({
            "model": "gemini-2.5-flash",
            "messages": [{"role": "user", "content": "Hi"}],
            "reasoning_effort": "low"
        });
        let result = translate(req);
        assert_eq!(
            result["generationConfig"]["thinkingConfig"]["thinkingBudget"],
            1024
        );
    }

    #[test]
    fn test_reasoning_effort_medium_to_thinking_config() {
        let req = json!({
            "model": "gemini-2.5-flash",
            "messages": [{"role": "user", "content": "Hi"}],
            "reasoning_effort": "medium"
        });
        let result = translate(req);
        assert_eq!(
            result["generationConfig"]["thinkingConfig"]["thinkingBudget"],
            4096
        );
    }

    #[test]
    fn test_reasoning_effort_high_to_thinking_config() {
        let req = json!({
            "model": "gemini-2.5-flash",
            "messages": [{"role": "user", "content": "Hi"}],
            "reasoning_effort": "high",
            "max_tokens": 16384
        });
        let result = translate(req);
        // high = max_tokens.max(8192) * 0.8 = 16384 * 0.8 = 13107
        assert_eq!(
            result["generationConfig"]["thinkingConfig"]["thinkingBudget"],
            13107
        );
    }

    #[test]
    fn test_thinking_budget_to_thinking_config() {
        let req = json!({
            "model": "gemini-2.5-flash",
            "messages": [{"role": "user", "content": "Hi"}],
            "thinking": {"type": "enabled", "budget_tokens": 10000}
        });
        let result = translate(req);
        assert_eq!(
            result["generationConfig"]["thinkingConfig"]["thinkingBudget"],
            10000
        );
    }

    // === Structured output (response_format) translation ===

    #[test]
    fn test_json_schema_to_response_schema() {
        let req = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "What is 2+2?"}],
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "math_response",
                    "schema": {
                        "type": "object",
                        "properties": {
                            "answer": {"type": "number"}
                        }
                    }
                }
            }
        });
        let result = translate(req);
        let gc = &result["generationConfig"];
        assert_eq!(gc["responseMimeType"], "application/json");
        assert_eq!(gc["responseSchema"]["type"], "object");
        assert_eq!(
            gc["responseSchema"]["properties"]["answer"]["type"],
            "number"
        );
    }

    #[test]
    fn test_json_object_to_response_mime_type() {
        let req = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Give me JSON"}],
            "response_format": {"type": "json_object"}
        });
        let result = translate(req);
        let gc = &result["generationConfig"];
        assert_eq!(gc["responseMimeType"], "application/json");
        assert!(gc.get("responseSchema").is_none());
    }

    #[test]
    fn test_missing_messages_error() {
        let req = json!({"model": "gpt-4"});
        let raw = serde_json::to_vec(&req).unwrap();
        let result = translate_request("gemini-1.5-pro", &raw, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_content_gets_placeholder() {
        let req = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": null}]
        });
        let result = translate(req);
        assert_eq!(result["contents"][0]["parts"][0]["text"], "");
    }
}
