use crate::TranslateState;
use prism_types::error::ProxyError;
use serde_json::{Value, json};

/// Map OpenAI finish_reason to Gemini finishReason.
fn map_openai_finish_reason(reason: Option<&str>) -> &'static str {
    match reason {
        Some("stop") => "STOP",
        Some("length") => "MAX_TOKENS",
        Some("tool_calls") => "STOP",
        Some("content_filter") => "SAFETY",
        _ => "STOP",
    }
}

pub fn translate_non_stream(
    _model: &str,
    _original_req: &[u8],
    data: &[u8],
) -> Result<String, ProxyError> {
    let resp: Value = serde_json::from_slice(data)?;

    // Extract first choice
    let choice = resp
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first());

    let (parts, finish_reason) = if let Some(choice) = choice {
        let message = choice.get("message");
        let mut parts = Vec::new();

        // Extract text content
        if let Some(content) = message
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
        {
            parts.push(json!({"text": content}));
        }

        // Extract tool_calls → functionCall parts
        if let Some(tool_calls) = message
            .and_then(|m| m.get("tool_calls"))
            .and_then(|tc| tc.as_array())
        {
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
                let args: Value = serde_json::from_str(arguments_str).unwrap_or(json!({}));

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

        let finish = map_openai_finish_reason(choice.get("finish_reason").and_then(|f| f.as_str()));

        (parts, finish)
    } else {
        (vec![json!({"text": ""})], "STOP")
    };

    let mut gemini_resp = json!({
        "candidates": [{
            "content": {
                "role": "model",
                "parts": parts,
            },
            "finishReason": finish_reason,
        }],
    });

    // Map usage
    if let Some(usage) = resp.get("usage") {
        let prompt = usage
            .get("prompt_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let completion = usage
            .get("completion_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let total = usage
            .get("total_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(prompt + completion);

        gemini_resp["usageMetadata"] = json!({
            "promptTokenCount": prompt,
            "candidatesTokenCount": completion,
            "totalTokenCount": total,
        });
    }

    serde_json::to_string(&gemini_resp).map_err(|e| ProxyError::Translation(e.to_string()))
}

pub fn translate_stream(
    _model: &str,
    _original_req: &[u8],
    _event_type: Option<&str>,
    data: &[u8],
    _state: &mut TranslateState,
) -> Result<Vec<String>, ProxyError> {
    let chunk: Value = serde_json::from_slice(data)?;
    let mut results = Vec::new();

    let choice = chunk
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first());

    if let Some(choice) = choice {
        let delta = choice.get("delta");
        let mut parts = Vec::new();

        // Text content
        if let Some(content) = delta
            .and_then(|d| d.get("content"))
            .and_then(|c| c.as_str())
            && !content.is_empty()
        {
            parts.push(json!({"text": content}));
        }

        // Tool calls → functionCall parts
        if let Some(tool_calls) = delta
            .and_then(|d| d.get("tool_calls"))
            .and_then(|tc| tc.as_array())
        {
            for tc in tool_calls {
                // Only emit functionCall when we have a name (initial tool call chunk)
                if let Some(name) = tc
                    .get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                {
                    let args_str = tc
                        .get("function")
                        .and_then(|f| f.get("arguments"))
                        .and_then(|a| a.as_str())
                        .unwrap_or("{}");
                    let args: Value = serde_json::from_str(args_str).unwrap_or(json!({}));
                    parts.push(json!({
                        "functionCall": {
                            "name": name,
                            "args": args,
                        }
                    }));
                }
            }
        }

        let finish_reason = choice.get("finish_reason").and_then(|f| f.as_str());

        // Skip role-only or empty chunks (no content, no finish)
        if parts.is_empty() && finish_reason.is_none() {
            return Ok(results);
        }

        let mut candidate = json!({
            "content": {
                "role": "model",
                "parts": if parts.is_empty() { vec![json!({"text": ""})] } else { parts },
            },
        });

        if let Some(reason) = finish_reason {
            candidate["finishReason"] = json!(map_openai_finish_reason(Some(reason)));
        }

        let mut gemini_chunk = json!({"candidates": [candidate]});

        // Include usage if available
        if let Some(usage) = chunk.get("usage") {
            let prompt = usage
                .get("prompt_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let completion = usage
                .get("completion_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            gemini_chunk["usageMetadata"] = json!({
                "promptTokenCount": prompt,
                "candidatesTokenCount": completion,
                "totalTokenCount": prompt + completion,
            });
        }

        results.push(serde_json::to_string(&gemini_chunk)?);
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================
    // Non-stream tests
    // ==================

    #[test]
    fn test_non_stream_basic_text() {
        let openai_resp = json!({
            "id": "chatcmpl-xxx",
            "object": "chat.completion",
            "model": "gpt-4",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "Hello there!"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        });
        let data = serde_json::to_vec(&openai_resp).unwrap();
        let result: Value =
            serde_json::from_str(&translate_non_stream("model", b"{}", &data).unwrap()).unwrap();

        assert_eq!(result["candidates"][0]["content"]["role"], "model");
        assert_eq!(
            result["candidates"][0]["content"]["parts"][0]["text"],
            "Hello there!"
        );
        assert_eq!(result["candidates"][0]["finishReason"], "STOP");
        assert_eq!(result["usageMetadata"]["promptTokenCount"], 10);
        assert_eq!(result["usageMetadata"]["candidatesTokenCount"], 5);
        assert_eq!(result["usageMetadata"]["totalTokenCount"], 15);
    }

    #[test]
    fn test_non_stream_tool_calls() {
        let openai_resp = json!({
            "id": "chatcmpl-xxx",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {"name": "get_weather", "arguments": "{\"city\":\"SF\"}"}
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });
        let data = serde_json::to_vec(&openai_resp).unwrap();
        let result: Value =
            serde_json::from_str(&translate_non_stream("model", b"{}", &data).unwrap()).unwrap();

        let fc = &result["candidates"][0]["content"]["parts"][0]["functionCall"];
        assert_eq!(fc["name"], "get_weather");
        assert_eq!(fc["args"]["city"], "SF");
        assert_eq!(result["candidates"][0]["finishReason"], "STOP");
    }

    #[test]
    fn test_non_stream_mixed_content_and_tool_calls() {
        let openai_resp = json!({
            "id": "chatcmpl-xxx",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "Let me check.",
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {"name": "search", "arguments": "{}"}
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });
        let data = serde_json::to_vec(&openai_resp).unwrap();
        let result: Value =
            serde_json::from_str(&translate_non_stream("model", b"{}", &data).unwrap()).unwrap();

        let parts = result["candidates"][0]["content"]["parts"]
            .as_array()
            .unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0]["text"], "Let me check.");
        assert_eq!(parts[1]["functionCall"]["name"], "search");
    }

    #[test]
    fn test_non_stream_usage() {
        let openai_resp = json!({
            "id": "chatcmpl-xxx",
            "choices": [{
                "message": {"role": "assistant", "content": "Hi"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 20, "completion_tokens": 10, "total_tokens": 30}
        });
        let data = serde_json::to_vec(&openai_resp).unwrap();
        let result: Value =
            serde_json::from_str(&translate_non_stream("model", b"{}", &data).unwrap()).unwrap();

        assert_eq!(result["usageMetadata"]["promptTokenCount"], 20);
        assert_eq!(result["usageMetadata"]["candidatesTokenCount"], 10);
        assert_eq!(result["usageMetadata"]["totalTokenCount"], 30);
    }

    #[test]
    fn test_non_stream_no_usage() {
        let openai_resp = json!({
            "id": "chatcmpl-xxx",
            "choices": [{
                "message": {"role": "assistant", "content": "Hi"},
                "finish_reason": "stop"
            }]
        });
        let data = serde_json::to_vec(&openai_resp).unwrap();
        let result: Value =
            serde_json::from_str(&translate_non_stream("model", b"{}", &data).unwrap()).unwrap();

        assert!(result.get("usageMetadata").is_none());
    }

    #[test]
    fn test_non_stream_finish_reason_mapping() {
        let test_cases = vec![
            ("stop", "STOP"),
            ("length", "MAX_TOKENS"),
            ("tool_calls", "STOP"),
            ("content_filter", "SAFETY"),
        ];
        for (openai_reason, expected) in test_cases {
            let openai_resp = json!({
                "id": "chatcmpl-xxx",
                "choices": [{
                    "message": {"role": "assistant", "content": "Hi"},
                    "finish_reason": openai_reason
                }]
            });
            let data = serde_json::to_vec(&openai_resp).unwrap();
            let result: Value =
                serde_json::from_str(&translate_non_stream("model", b"{}", &data).unwrap())
                    .unwrap();
            assert_eq!(
                result["candidates"][0]["finishReason"], expected,
                "OpenAI '{openai_reason}' should map to '{expected}'"
            );
        }
    }

    #[test]
    fn test_non_stream_no_choices() {
        let openai_resp = json!({"id": "chatcmpl-xxx"});
        let data = serde_json::to_vec(&openai_resp).unwrap();
        let result: Value =
            serde_json::from_str(&translate_non_stream("model", b"{}", &data).unwrap()).unwrap();

        assert_eq!(result["candidates"][0]["content"]["parts"][0]["text"], "");
        assert_eq!(result["candidates"][0]["finishReason"], "STOP");
    }

    // ==================
    // Stream tests
    // ==================

    fn new_state() -> TranslateState {
        TranslateState::default()
    }

    #[test]
    fn test_stream_basic() {
        let mut state = new_state();
        let chunk = json!({
            "id": "chatcmpl-xxx",
            "object": "chat.completion.chunk",
            "model": "gpt-4",
            "choices": [{
                "index": 0,
                "delta": {"content": "Hello"},
                "finish_reason": Value::Null,
            }]
        });
        let data = serde_json::to_vec(&chunk).unwrap();
        let results = translate_stream("model", b"{}", None, &data, &mut state).unwrap();

        assert_eq!(results.len(), 1);
        let result: Value = serde_json::from_str(&results[0]).unwrap();
        assert_eq!(
            result["candidates"][0]["content"]["parts"][0]["text"],
            "Hello"
        );
    }

    #[test]
    fn test_stream_role_only_chunk_skipped() {
        let mut state = new_state();
        let chunk = json!({
            "id": "chatcmpl-xxx",
            "choices": [{
                "delta": {"role": "assistant", "content": ""},
                "finish_reason": Value::Null,
            }]
        });
        let data = serde_json::to_vec(&chunk).unwrap();
        let results = translate_stream("model", b"{}", None, &data, &mut state).unwrap();

        // Role-only chunk with empty content should be skipped
        assert!(results.is_empty());
    }

    #[test]
    fn test_stream_finish_reason() {
        let mut state = new_state();
        let chunk = json!({
            "id": "chatcmpl-xxx",
            "choices": [{
                "delta": {},
                "finish_reason": "stop",
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        });
        let data = serde_json::to_vec(&chunk).unwrap();
        let results = translate_stream("model", b"{}", None, &data, &mut state).unwrap();

        assert_eq!(results.len(), 1);
        let result: Value = serde_json::from_str(&results[0]).unwrap();
        assert_eq!(result["candidates"][0]["finishReason"], "STOP");
        assert_eq!(result["usageMetadata"]["promptTokenCount"], 10);
        assert_eq!(result["usageMetadata"]["candidatesTokenCount"], 5);
    }

    #[test]
    fn test_stream_tool_call_with_name() {
        let mut state = new_state();
        let chunk = json!({
            "id": "chatcmpl-xxx",
            "choices": [{
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "id": "call_1",
                        "type": "function",
                        "function": {"name": "weather", "arguments": "{\"city\":\"SF\"}"}
                    }]
                },
                "finish_reason": Value::Null,
            }]
        });
        let data = serde_json::to_vec(&chunk).unwrap();
        let results = translate_stream("model", b"{}", None, &data, &mut state).unwrap();

        assert_eq!(results.len(), 1);
        let result: Value = serde_json::from_str(&results[0]).unwrap();
        let fc = &result["candidates"][0]["content"]["parts"][0]["functionCall"];
        assert_eq!(fc["name"], "weather");
        assert_eq!(fc["args"]["city"], "SF");
    }

    #[test]
    fn test_stream_empty_chunk_skipped() {
        let mut state = new_state();
        let chunk = json!({
            "id": "chatcmpl-xxx",
            "choices": [{
                "delta": {},
                "finish_reason": Value::Null,
            }]
        });
        let data = serde_json::to_vec(&chunk).unwrap();
        let results = translate_stream("model", b"{}", None, &data, &mut state).unwrap();
        assert!(results.is_empty());
    }
}
