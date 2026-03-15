use crate::common;
use async_trait::async_trait;
use prism_core::error::ProxyError;
use prism_core::provider::*;
use prism_core::proxy::HttpClientPool;
use serde_json::{Value, json};
use std::sync::Arc;

pub struct OpenAICompatExecutor {
    pub name: String,
    pub format: Format,
    pub global_proxy: Option<String>,
    pub client_pool: Arc<HttpClientPool>,
}

impl OpenAICompatExecutor {
    /// Build a POST request with Bearer auth and standard headers.
    fn build_request(
        &self,
        auth: &AuthRecord,
        url: &str,
        body: &[u8],
        request_headers: &std::collections::HashMap<String, String>,
    ) -> Result<reqwest::RequestBuilder, ProxyError> {
        let client = common::build_client(auth, self.global_proxy.as_deref(), &self.client_pool)?;
        let req = client
            .post(url)
            .header("content-type", "application/json")
            .body(body.to_vec());
        let req = common::apply_auth(req, auth);
        Ok(common::apply_headers(req, request_headers, auth))
    }
}

/// Check if the auth record uses the Responses API wire format.
fn use_responses_api(auth: &AuthRecord) -> bool {
    auth.wire_api == prism_core::provider::WireApi::Responses
}

fn extract_text_content(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Array(parts) => {
            let text = parts
                .iter()
                .filter_map(
                    |part| match part.get("type").and_then(|value| value.as_str()) {
                        Some("text") | Some("input_text") | Some("output_text") => part
                            .get("text")
                            .and_then(|value| value.as_str())
                            .map(ToOwned::to_owned),
                        _ => None,
                    },
                )
                .collect::<String>();
            (!text.is_empty()).then_some(text)
        }
        _ => None,
    }
}

fn normalize_chat_content_part(part: &Value) -> Value {
    let part_type = part
        .get("type")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    match part_type {
        "text" => json!({
            "type": "input_text",
            "text": part.get("text").and_then(|value| value.as_str()).unwrap_or(""),
        }),
        "image_url" => {
            let url = part
                .get("image_url")
                .and_then(|value| value.get("url").or(Some(value)))
                .and_then(|value| value.as_str())
                .unwrap_or("");
            json!({
                "type": "input_image",
                "image_url": url,
            })
        }
        "input_text" | "input_image" | "input_file" | "computer_screenshot" => part.clone(),
        _ => part.clone(),
    }
}

fn normalize_chat_content(content: Option<&Value>) -> Value {
    match content {
        Some(Value::String(text)) => json!([{
            "type": "input_text",
            "text": text,
        }]),
        Some(Value::Array(parts)) => Value::Array(
            parts
                .iter()
                .map(normalize_chat_content_part)
                .collect::<Vec<_>>(),
        ),
        Some(Value::Null) | None => Value::Array(Vec::new()),
        Some(other) => other.clone(),
    }
}

fn content_has_parts(content: &Value) -> bool {
    match content {
        Value::Array(parts) => !parts.is_empty(),
        Value::String(text) => !text.is_empty(),
        Value::Null => false,
        _ => true,
    }
}

fn normalize_tool_definition(tool: &Value) -> Value {
    let function = tool.get("function").unwrap_or(tool);
    let mut normalized = json!({
        "type": tool.get("type").and_then(|value| value.as_str()).unwrap_or("function"),
        "name": function.get("name").and_then(|value| value.as_str()).unwrap_or(""),
    });
    if let Some(description) = function.get("description").cloned() {
        normalized["description"] = description;
    }
    if let Some(parameters) = function.get("parameters").cloned() {
        normalized["parameters"] = parameters;
    }
    if let Some(strict) = function.get("strict").cloned() {
        normalized["strict"] = strict;
    }
    normalized
}

fn normalize_tool_choice(tool_choice: Value) -> Value {
    if tool_choice
        .get("type")
        .and_then(|value| value.as_str())
        .is_some_and(|value| value == "function")
        && let Some(name) = tool_choice
            .get("function")
            .and_then(|value| value.get("name"))
            .and_then(|value| value.as_str())
    {
        return json!({
            "type": "function",
            "name": name,
        });
    }
    tool_choice
}

fn stringify_tool_output(content: Option<&Value>) -> Value {
    match content {
        Some(Value::String(text)) => Value::String(text.clone()),
        Some(Value::Array(parts)) => extract_text_content(&Value::Array(parts.clone()))
            .map(Value::String)
            .unwrap_or_else(|| Value::String(Value::Array(parts.clone()).to_string())),
        Some(Value::Null) | None => Value::String(String::new()),
        Some(other) => Value::String(other.to_string()),
    }
}

fn response_tool_call(item: &Value) -> Option<Value> {
    if item.get("type").and_then(|value| value.as_str()) != Some("function_call") {
        return None;
    }
    Some(json!({
        "id": item
            .get("call_id")
            .and_then(|value| value.as_str())
            .or_else(|| item.get("id").and_then(|value| value.as_str()))
            .unwrap_or(""),
        "type": "function",
        "function": {
            "name": item.get("name").and_then(|value| value.as_str()).unwrap_or(""),
            "arguments": item
                .get("arguments")
                .and_then(|value| value.as_str())
                .unwrap_or("{}"),
        }
    }))
}

/// Convert a Chat Completions request body to Responses API format.
pub(crate) fn chat_to_responses(payload: &[u8]) -> Result<Vec<u8>, ProxyError> {
    let mut v: Value =
        serde_json::from_slice(payload).map_err(|e| ProxyError::BadRequest(e.to_string()))?;

    let obj = v
        .as_object_mut()
        .ok_or_else(|| ProxyError::BadRequest("expected JSON object".into()))?;

    // messages -> input
    if let Some(messages) = obj.remove("messages") {
        // Extract system messages as instructions
        if let Some(arr) = messages.as_array() {
            let mut instructions: Vec<String> = Vec::new();
            let mut input = Vec::new();
            for msg in arr {
                match msg
                    .get("role")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                {
                    "system" => {
                        if let Some(text) = msg.get("content").and_then(extract_text_content) {
                            instructions.push(text);
                        }
                    }
                    "tool" => {
                        input.push(json!({
                            "type": "function_call_output",
                            "call_id": msg
                                .get("tool_call_id")
                                .and_then(|value| value.as_str())
                                .unwrap_or(""),
                            "output": stringify_tool_output(msg.get("content")),
                        }));
                    }
                    "assistant" => {
                        let normalized_content = normalize_chat_content(msg.get("content"));
                        if content_has_parts(&normalized_content) {
                            input.push(json!({
                                "role": "assistant",
                                "content": normalized_content,
                            }));
                        }
                        if let Some(tool_calls) =
                            msg.get("tool_calls").and_then(|value| value.as_array())
                        {
                            for tool_call in tool_calls {
                                input.push(json!({
                                    "type": "function_call",
                                    "call_id": tool_call
                                        .get("id")
                                        .and_then(|value| value.as_str())
                                        .unwrap_or(""),
                                    "name": tool_call
                                        .get("function")
                                        .and_then(|value| value.get("name"))
                                        .and_then(|value| value.as_str())
                                        .unwrap_or(""),
                                    "arguments": tool_call
                                        .get("function")
                                        .and_then(|value| value.get("arguments"))
                                        .and_then(|value| value.as_str())
                                        .unwrap_or("{}"),
                                }));
                            }
                        }
                    }
                    role => {
                        input.push(json!({
                            "role": role,
                            "content": normalize_chat_content(msg.get("content")),
                        }));
                    }
                }
            }
            if !instructions.is_empty() && !obj.contains_key("instructions") {
                obj.insert(
                    "instructions".into(),
                    Value::String(instructions.join("\n")),
                );
            }
            obj.insert("input".into(), Value::Array(input));
        } else {
            obj.insert("input".into(), messages);
        }
    }

    if let Some(tools) = obj.get_mut("tools").and_then(|value| value.as_array_mut()) {
        let normalized = tools
            .iter()
            .map(normalize_tool_definition)
            .collect::<Vec<_>>();
        *tools = normalized;
    }

    if let Some(tool_choice) = obj.remove("tool_choice") {
        obj.insert("tool_choice".into(), normalize_tool_choice(tool_choice));
    }

    // max_tokens -> max_output_tokens
    if let Some(max_tokens) = obj.remove("max_tokens")
        && !obj.contains_key("max_output_tokens")
    {
        obj.insert("max_output_tokens".into(), max_tokens);
    }

    // Remove Chat Completions-specific fields that Responses API doesn't accept
    obj.remove("stream");

    serde_json::to_vec(obj).map_err(|e| ProxyError::Internal(e.to_string()))
}

/// Convert a Responses API response to Chat Completions format.
pub(crate) fn responses_to_chat(payload: &[u8]) -> Result<bytes::Bytes, ProxyError> {
    let v: Value =
        serde_json::from_slice(payload).map_err(|e| ProxyError::Internal(e.to_string()))?;

    // Extract content from output[].content[].text
    let mut content = String::new();
    let mut tool_calls = Vec::new();
    if let Some(output) = v.get("output").and_then(|o| o.as_array()) {
        for item in output {
            if item.get("type").and_then(|t| t.as_str()) == Some("message")
                && let Some(contents) = item.get("content").and_then(|c| c.as_array())
            {
                for c in contents {
                    if c.get("type").and_then(|t| t.as_str()) == Some("output_text")
                        && let Some(text) = c.get("text").and_then(|t| t.as_str())
                    {
                        content.push_str(text);
                    }
                }
            }
            if let Some(tool_call) = response_tool_call(item) {
                tool_calls.push(tool_call);
            }
        }
    }

    let model = v.get("model").and_then(|m| m.as_str()).unwrap_or("unknown");
    let id = v.get("id").and_then(|i| i.as_str()).unwrap_or("");
    let created = v.get("created_at").and_then(|c| c.as_u64()).unwrap_or(0);

    // Extract usage
    let usage = v
        .get("usage")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let prompt_tokens = usage
        .get("input_tokens")
        .and_then(|t| t.as_u64())
        .unwrap_or(0);
    let completion_tokens = usage
        .get("output_tokens")
        .and_then(|t| t.as_u64())
        .unwrap_or(0);

    let finish_reason = match v.get("status").and_then(|s| s.as_str()) {
        _ if !tool_calls.is_empty() => "tool_calls",
        Some("completed") => "stop",
        Some("incomplete") => "length",
        _ => "stop",
    };

    let content_value = match (!content.is_empty(), tool_calls.is_empty()) {
        (true, _) => Value::String(content),
        (false, false) => Value::Null,
        (false, true) => Value::String(String::new()),
    };

    let mut message = json!({
        "role": "assistant",
        "content": content_value,
    });
    if !tool_calls.is_empty() {
        message["tool_calls"] = Value::Array(tool_calls);
    }

    let chat_response = json!({
        "id": format!("chatcmpl-{id}"),
        "object": "chat.completion",
        "created": created,
        "model": model,
        "choices": [{
            "index": 0,
            "message": message,
            "finish_reason": finish_reason,
        }],
        "usage": {
            "prompt_tokens": prompt_tokens,
            "completion_tokens": completion_tokens,
            "total_tokens": prompt_tokens + completion_tokens,
        }
    });

    serde_json::to_vec(&chat_response)
        .map(bytes::Bytes::from)
        .map_err(|e| ProxyError::Internal(e.to_string()))
}

pub(crate) fn synthesize_chat_stream_chunks(
    payload: &Value,
) -> Result<Vec<Result<StreamChunk, ProxyError>>, ProxyError> {
    let choice = payload
        .get("choices")
        .and_then(|value| value.get(0))
        .ok_or_else(|| ProxyError::Internal("chat response missing choices[0]".into()))?;
    let message = choice
        .get("message")
        .ok_or_else(|| ProxyError::Internal("chat response missing choices[0].message".into()))?;
    let content = message
        .get("content")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let tool_calls = message
        .get("tool_calls")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let model = payload
        .get("model")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let id = payload
        .get("id")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let created = payload
        .get("created")
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    let finish_reason = if !tool_calls.is_empty() {
        "tool_calls"
    } else {
        choice
            .get("finish_reason")
            .and_then(|value| value.as_str())
            .unwrap_or("stop")
    };

    let role_chunk = json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": {"role": "assistant", "content": ""},
            "finish_reason": null
        }]
    });

    let mut chunks = vec![Ok(StreamChunk {
        event_type: None,
        data: role_chunk.to_string(),
    })];

    if !content.is_empty() {
        let content_chunk = json!({
            "id": id,
            "object": "chat.completion.chunk",
            "created": created,
            "model": model,
            "choices": [{
                "index": 0,
                "delta": {"content": content},
                "finish_reason": null
            }]
        });
        chunks.push(Ok(StreamChunk {
            event_type: None,
            data: content_chunk.to_string(),
        }));
    }

    for (index, tool_call) in tool_calls.iter().enumerate() {
        let tool_chunk = json!({
            "id": id,
            "object": "chat.completion.chunk",
            "created": created,
            "model": model,
            "choices": [{
                "index": 0,
                "delta": {
                    "tool_calls": [{
                        "index": index,
                        "id": tool_call.get("id").and_then(|value| value.as_str()).unwrap_or(""),
                        "type": "function",
                        "function": {
                            "name": tool_call
                                .get("function")
                                .and_then(|value| value.get("name"))
                                .and_then(|value| value.as_str())
                                .unwrap_or(""),
                            "arguments": tool_call
                                .get("function")
                                .and_then(|value| value.get("arguments"))
                                .and_then(|value| value.as_str())
                                .unwrap_or("{}"),
                        }
                    }]
                },
                "finish_reason": null
            }]
        });
        chunks.push(Ok(StreamChunk {
            event_type: None,
            data: tool_chunk.to_string(),
        }));
    }

    let usage = payload.get("usage").cloned().unwrap_or_else(|| json!({}));
    let stop_chunk = json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": {},
            "finish_reason": finish_reason
        }],
        "usage": usage,
    });
    chunks.push(Ok(StreamChunk {
        event_type: None,
        data: stop_chunk.to_string(),
    }));
    chunks.push(Ok(StreamChunk {
        event_type: None,
        data: "[DONE]".to_string(),
    }));

    Ok(chunks)
}

#[async_trait]
impl ProviderExecutor for OpenAICompatExecutor {
    fn identifier(&self) -> &str {
        &self.name
    }

    fn native_format(&self) -> Format {
        self.format
    }

    async fn execute(
        &self,
        auth: &AuthRecord,
        request: ProviderRequest,
    ) -> Result<ProviderResponse, ProxyError> {
        let base_url = auth.resolved_base_url();

        let (url, body) = if request.responses_passthrough {
            // Body is already in Responses API format — forward as-is
            (format!("{base_url}/v1/responses"), request.payload.to_vec())
        } else if use_responses_api(auth) {
            (
                format!("{base_url}/v1/responses"),
                chat_to_responses(&request.payload)?,
            )
        } else {
            (
                format!("{base_url}/v1/chat/completions"),
                request.payload.to_vec(),
            )
        };

        let req = self.build_request(auth, &url, &body, &request.headers)?;
        let (resp_body, headers) = common::handle_response(req.send().await?).await?;

        // Convert response back to Chat Completions format (unless passthrough)
        let payload = if request.responses_passthrough {
            resp_body
        } else if use_responses_api(auth) {
            responses_to_chat(&resp_body)?
        } else {
            resp_body
        };

        Ok(ProviderResponse { payload, headers })
    }

    async fn execute_stream(
        &self,
        auth: &AuthRecord,
        request: ProviderRequest,
    ) -> Result<StreamResult, ProxyError> {
        if request.responses_passthrough {
            // Body is already in Responses API format — forward to /v1/responses for streaming
            let base_url = auth.resolved_base_url();
            let url = format!("{base_url}/v1/responses");
            let req = self.build_request(auth, &url, &request.payload, &request.headers)?;
            return common::handle_stream_response(req.send().await?).await;
        }

        if use_responses_api(auth) {
            // Responses API: execute non-streaming, then emit as streaming chunks.
            let response = self.execute(auth, request).await?;
            let v: Value = serde_json::from_slice(&response.payload)
                .map_err(|e| ProxyError::Internal(e.to_string()))?;
            let chunks = synthesize_chat_stream_chunks(&v)?;
            return Ok(StreamResult {
                headers: response.headers,
                stream: Box::pin(futures::stream::iter(chunks)),
            });
        }

        let base_url = auth.resolved_base_url();
        let url = format!("{base_url}/v1/chat/completions");

        let req = self.build_request(auth, &url, &request.payload, &request.headers)?;
        common::handle_stream_response(req.send().await?).await
    }

    fn supported_models(&self, auth: &AuthRecord) -> Vec<ModelInfo> {
        common::supported_models_from_auth(auth, &self.name, &self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === chat_to_responses ===

    #[test]
    fn test_chat_to_responses_basic() {
        let chat_req = json!({
            "model": "gpt-4o",
            "messages": [
                {"role": "user", "content": "Hello"}
            ],
            "stream": true
        });
        let payload = serde_json::to_vec(&chat_req).unwrap();
        let result: serde_json::Value =
            serde_json::from_slice(&chat_to_responses(&payload).unwrap()).unwrap();

        // messages -> input
        assert_eq!(result["input"][0]["role"], "user");
        assert_eq!(result["input"][0]["content"][0]["type"], "input_text");
        assert_eq!(result["input"][0]["content"][0]["text"], "Hello");
        // stream should be removed
        assert!(result.get("stream").is_none());
        // model should be preserved
        assert_eq!(result["model"], "gpt-4o");
    }

    #[test]
    fn test_chat_to_responses_system_to_instructions() {
        let chat_req = json!({
            "model": "gpt-4o",
            "messages": [
                {"role": "system", "content": "Be concise."},
                {"role": "user", "content": "Hello"}
            ]
        });
        let payload = serde_json::to_vec(&chat_req).unwrap();
        let result: serde_json::Value =
            serde_json::from_slice(&chat_to_responses(&payload).unwrap()).unwrap();

        assert_eq!(result["instructions"], "Be concise.");
        // System message should be filtered from input
        assert_eq!(result["input"].as_array().unwrap().len(), 1);
        assert_eq!(result["input"][0]["role"], "user");
    }

    #[test]
    fn test_chat_to_responses_multiple_system_messages() {
        let chat_req = json!({
            "model": "gpt-4o",
            "messages": [
                {"role": "system", "content": "Rule 1"},
                {"role": "system", "content": "Rule 2"},
                {"role": "user", "content": "Hi"}
            ]
        });
        let payload = serde_json::to_vec(&chat_req).unwrap();
        let result: serde_json::Value =
            serde_json::from_slice(&chat_to_responses(&payload).unwrap()).unwrap();

        assert_eq!(result["instructions"], "Rule 1\nRule 2");
    }

    #[test]
    fn test_chat_to_responses_max_tokens() {
        let chat_req = json!({
            "model": "gpt-4o",
            "messages": [{"role": "user", "content": "Hi"}],
            "max_tokens": 1024
        });
        let payload = serde_json::to_vec(&chat_req).unwrap();
        let result: serde_json::Value =
            serde_json::from_slice(&chat_to_responses(&payload).unwrap()).unwrap();

        assert_eq!(result["max_output_tokens"], 1024);
        assert!(result.get("max_tokens").is_none());
    }

    #[test]
    fn test_chat_to_responses_preserves_existing_instructions() {
        let chat_req = json!({
            "model": "gpt-4o",
            "messages": [
                {"role": "system", "content": "system msg"},
                {"role": "user", "content": "Hi"}
            ],
            "instructions": "existing instructions"
        });
        let payload = serde_json::to_vec(&chat_req).unwrap();
        let result: serde_json::Value =
            serde_json::from_slice(&chat_to_responses(&payload).unwrap()).unwrap();

        // Should NOT overwrite existing instructions
        assert_eq!(result["instructions"], "existing instructions");
    }

    #[test]
    fn test_chat_to_responses_converts_images_and_tools() {
        let chat_req = json!({
            "model": "gpt-4o",
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "text", "text": "Describe the image"},
                    {"type": "image_url", "image_url": {"url": "https://example.com/test.png"}}
                ]
            }],
            "tools": [{
                "type": "function",
                "function": {
                    "name": "probe_tool",
                    "description": "Probe tool",
                    "parameters": {"type": "object"}
                }
            }],
            "tool_choice": {"type": "function", "function": {"name": "probe_tool"}}
        });
        let payload = serde_json::to_vec(&chat_req).unwrap();
        let result: Value = serde_json::from_slice(&chat_to_responses(&payload).unwrap()).unwrap();

        assert_eq!(result["input"][0]["content"][0]["type"], "input_text");
        assert_eq!(
            result["input"][0]["content"][0]["text"],
            "Describe the image"
        );
        assert_eq!(result["input"][0]["content"][1]["type"], "input_image");
        assert_eq!(
            result["input"][0]["content"][1]["image_url"],
            "https://example.com/test.png"
        );
        assert_eq!(result["tools"][0]["type"], "function");
        assert_eq!(result["tools"][0]["name"], "probe_tool");
        assert_eq!(result["tool_choice"]["type"], "function");
        assert_eq!(result["tool_choice"]["name"], "probe_tool");
    }

    #[test]
    fn test_chat_to_responses_converts_tool_messages() {
        let chat_req = json!({
            "model": "gpt-4o",
            "messages": [
                {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_123",
                        "type": "function",
                        "function": {"name": "probe_tool", "arguments": "{\"x\":1}"}
                    }]
                },
                {
                    "role": "tool",
                    "tool_call_id": "call_123",
                    "content": "{\"ok\":true}"
                }
            ]
        });
        let payload = serde_json::to_vec(&chat_req).unwrap();
        let result: Value = serde_json::from_slice(&chat_to_responses(&payload).unwrap()).unwrap();

        assert_eq!(result["input"][0]["type"], "function_call");
        assert_eq!(result["input"][0]["call_id"], "call_123");
        assert_eq!(result["input"][0]["name"], "probe_tool");
        assert_eq!(result["input"][0]["arguments"], "{\"x\":1}");
        assert_eq!(result["input"][1]["type"], "function_call_output");
        assert_eq!(result["input"][1]["call_id"], "call_123");
        assert_eq!(result["input"][1]["output"], "{\"ok\":true}");
    }

    // === responses_to_chat ===

    #[test]
    fn test_responses_to_chat_basic() {
        let responses_resp = json!({
            "id": "resp_123",
            "model": "gpt-4o-2024-08-06",
            "created_at": 1700000000u64,
            "status": "completed",
            "output": [{
                "type": "message",
                "content": [{
                    "type": "output_text",
                    "text": "Hello!"
                }]
            }],
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5
            }
        });
        let payload = serde_json::to_vec(&responses_resp).unwrap();
        let result: serde_json::Value =
            serde_json::from_slice(&responses_to_chat(&payload).unwrap()).unwrap();

        assert_eq!(result["id"], "chatcmpl-resp_123");
        assert_eq!(result["object"], "chat.completion");
        assert_eq!(result["model"], "gpt-4o-2024-08-06");
        assert_eq!(result["choices"][0]["message"]["role"], "assistant");
        assert_eq!(result["choices"][0]["message"]["content"], "Hello!");
        assert_eq!(result["choices"][0]["finish_reason"], "stop");
        assert_eq!(result["usage"]["prompt_tokens"], 10);
        assert_eq!(result["usage"]["completion_tokens"], 5);
        assert_eq!(result["usage"]["total_tokens"], 15);
    }

    #[test]
    fn test_responses_to_chat_incomplete() {
        let responses_resp = json!({
            "id": "resp_456",
            "model": "gpt-4o",
            "status": "incomplete",
            "output": [{
                "type": "message",
                "content": [{"type": "output_text", "text": "Partial"}]
            }],
            "usage": {"input_tokens": 10, "output_tokens": 100}
        });
        let payload = serde_json::to_vec(&responses_resp).unwrap();
        let result: serde_json::Value =
            serde_json::from_slice(&responses_to_chat(&payload).unwrap()).unwrap();

        assert_eq!(result["choices"][0]["finish_reason"], "length");
    }

    #[test]
    fn test_responses_to_chat_empty_output() {
        let responses_resp = json!({
            "id": "resp_789",
            "model": "gpt-4o",
            "status": "completed",
            "output": [],
            "usage": {"input_tokens": 5, "output_tokens": 0}
        });
        let payload = serde_json::to_vec(&responses_resp).unwrap();
        let result: serde_json::Value =
            serde_json::from_slice(&responses_to_chat(&payload).unwrap()).unwrap();

        assert_eq!(result["choices"][0]["message"]["content"], "");
    }

    #[test]
    fn test_responses_to_chat_multiple_content_blocks() {
        let responses_resp = json!({
            "id": "resp_multi",
            "model": "gpt-4o",
            "status": "completed",
            "output": [{
                "type": "message",
                "content": [
                    {"type": "output_text", "text": "Part 1"},
                    {"type": "output_text", "text": " Part 2"}
                ]
            }],
            "usage": {"input_tokens": 5, "output_tokens": 10}
        });
        let payload = serde_json::to_vec(&responses_resp).unwrap();
        let result: serde_json::Value =
            serde_json::from_slice(&responses_to_chat(&payload).unwrap()).unwrap();

        assert_eq!(result["choices"][0]["message"]["content"], "Part 1 Part 2");
    }

    #[test]
    fn test_responses_to_chat_maps_function_calls() {
        let responses_resp = json!({
            "id": "resp_tool",
            "model": "gpt-4o",
            "status": "completed",
            "output": [{
                "type": "function_call",
                "call_id": "call_123",
                "name": "probe_tool",
                "arguments": "{\"ok\":true}"
            }],
            "usage": {"input_tokens": 5, "output_tokens": 3}
        });
        let payload = serde_json::to_vec(&responses_resp).unwrap();
        let result: Value = serde_json::from_slice(&responses_to_chat(&payload).unwrap()).unwrap();

        assert!(result["choices"][0]["message"]["content"].is_null());
        assert_eq!(result["choices"][0]["finish_reason"], "tool_calls");
        assert_eq!(
            result["choices"][0]["message"]["tool_calls"][0]["id"],
            "call_123"
        );
        assert_eq!(
            result["choices"][0]["message"]["tool_calls"][0]["function"]["name"],
            "probe_tool"
        );
        assert_eq!(
            result["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"],
            "{\"ok\":true}"
        );
    }

    #[test]
    fn test_synthesize_chat_stream_chunks_with_tool_calls() {
        let chat_response = json!({
            "id": "chatcmpl-resp_tool",
            "created": 1700000000u64,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_123",
                        "type": "function",
                        "function": {"name": "probe_tool", "arguments": "{\"ok\":true}"}
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {"prompt_tokens": 5, "completion_tokens": 3, "total_tokens": 8}
        });

        let chunks = synthesize_chat_stream_chunks(&chat_response).unwrap();
        let serialized = chunks
            .into_iter()
            .map(|chunk| chunk.unwrap().data)
            .collect::<Vec<_>>();

        assert!(
            serialized
                .iter()
                .any(|chunk| chunk.contains("\"tool_calls\""))
        );
        assert!(
            serialized
                .iter()
                .any(|chunk| chunk.contains("\"probe_tool\""))
        );
        assert!(
            serialized
                .iter()
                .any(|chunk| chunk.contains("\"finish_reason\":\"tool_calls\""))
        );
        assert_eq!(serialized.last().unwrap(), "[DONE]");
    }
}
