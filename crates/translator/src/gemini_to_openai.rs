use crate::TranslateState;
use crate::common::{
    build_assistant_message, build_openai_chunk, build_openai_response, build_tool_call,
    build_tool_call_delta, map_gemini_finish_reason,
};
use prism_core::error::ProxyError;
use serde_json::{Value, json};

pub fn translate_non_stream(
    _model: &str,
    _original_req: &[u8],
    data: &[u8],
) -> Result<String, ProxyError> {
    let resp: Value = serde_json::from_slice(data)?;
    let created = chrono::Utc::now().timestamp();
    let id = format!("chatcmpl-{}", uuid::Uuid::new_v4());

    let model = resp
        .get("modelVersion")
        .and_then(|v| v.as_str())
        .unwrap_or("gemini")
        .to_string();

    // Extract first candidate
    let candidate = resp
        .get("candidates")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first());

    let (content_str, tool_calls, finish_reason) = if let Some(candidate) = candidate {
        let parts = candidate
            .get("content")
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.as_array());

        let mut text_parts = Vec::new();
        let mut tool_calls = Vec::new();
        let mut tc_index = 0u32;

        if let Some(parts) = parts {
            for part in parts {
                if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                    text_parts.push(text.to_string());
                } else if let Some(fc) = part.get("functionCall") {
                    let name = fc.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    let args = fc.get("args").cloned().unwrap_or(json!({}));
                    let arguments = serde_json::to_string(&args).unwrap_or_default();
                    let tc_id = format!("call_{}", uuid::Uuid::new_v4());

                    tool_calls.push(build_tool_call(&tc_id, name, &arguments, tc_index));
                    tc_index += 1;
                }
            }
        }

        let finish =
            map_gemini_finish_reason(candidate.get("finishReason").and_then(|v| v.as_str()));

        (text_parts.join(""), tool_calls, finish)
    } else {
        (String::new(), Vec::new(), "stop")
    };

    let content = if content_str.is_empty() {
        None
    } else {
        Some(content_str.as_str())
    };
    let tc = if tool_calls.is_empty() {
        None
    } else {
        Some(tool_calls)
    };
    let message = build_assistant_message(content, tc);

    // Map usage
    let usage = if let Some(u) = resp.get("usageMetadata") {
        let prompt = u
            .get("promptTokenCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let completion = u
            .get("candidatesTokenCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let total = u
            .get("totalTokenCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(prompt + completion);
        Some(json!({
            "prompt_tokens": prompt,
            "completion_tokens": completion,
            "total_tokens": total,
        }))
    } else {
        None
    };

    let openai_resp = build_openai_response(&id, created, &model, message, finish_reason, usage);
    serde_json::to_string(&openai_resp).map_err(|e| ProxyError::Translation(e.to_string()))
}

pub fn translate_stream(
    _model: &str,
    _original_req: &[u8],
    _event_type: Option<&str>,
    data: &[u8],
    state: &mut TranslateState,
) -> Result<Vec<String>, ProxyError> {
    let resp: Value = serde_json::from_slice(data)?;
    let mut chunks = Vec::new();

    // Initialize state if needed
    if state.response_id.is_empty() {
        state.response_id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
        state.created = chrono::Utc::now().timestamp();
        state.current_tool_call_index = -1;

        // Emit initial role chunk
        let chunk = build_openai_chunk(
            &state.response_id,
            state.created,
            &state.model,
            json!({"role": "assistant", "content": ""}),
            None,
        );
        chunks.push(serde_json::to_string(&chunk)?);
    }

    // Extract candidate
    let candidate = resp
        .get("candidates")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first());

    if let Some(candidate) = candidate {
        // Update model from response if available
        if let Some(model_ver) = resp.get("modelVersion").and_then(|v| v.as_str()) {
            state.model = model_ver.to_string();
        }

        let parts = candidate
            .get("content")
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.as_array());

        if let Some(parts) = parts {
            for part in parts {
                if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                    let chunk = build_openai_chunk(
                        &state.response_id,
                        state.created,
                        &state.model,
                        json!({"content": text}),
                        None,
                    );
                    chunks.push(serde_json::to_string(&chunk)?);
                } else if let Some(fc) = part.get("functionCall") {
                    state.current_tool_call_index += 1;
                    let name = fc.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    let args = fc.get("args").cloned().unwrap_or(json!({}));
                    let arguments = serde_json::to_string(&args).unwrap_or_default();
                    let tc_id = format!("call_{}", uuid::Uuid::new_v4());

                    let delta = json!({
                        "tool_calls": [build_tool_call_delta(
                            state.current_tool_call_index,
                            &tc_id,
                            name,
                            &arguments,
                        )],
                    });
                    let chunk = build_openai_chunk(
                        &state.response_id,
                        state.created,
                        &state.model,
                        delta,
                        None,
                    );
                    chunks.push(serde_json::to_string(&chunk)?);
                }
            }
        }

        // Check for finish_reason
        if let Some(finish) = candidate.get("finishReason").and_then(|v| v.as_str()) {
            let finish_reason = map_gemini_finish_reason(Some(finish));

            let mut chunk = build_openai_chunk(
                &state.response_id,
                state.created,
                &state.model,
                json!({}),
                Some(finish_reason),
            );

            // Include usage if available
            if let Some(u) = resp.get("usageMetadata") {
                let prompt = u
                    .get("promptTokenCount")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let completion = u
                    .get("candidatesTokenCount")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                chunk["usage"] = json!({
                    "prompt_tokens": prompt,
                    "completion_tokens": completion,
                    "total_tokens": prompt + completion,
                });
            }

            chunks.push(serde_json::to_string(&chunk)?);
            chunks.push("[DONE]".to_string());
        }
    }

    Ok(chunks)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================
    // Non-stream tests
    // ==================

    #[test]
    fn test_non_stream_basic_text() {
        let gemini_resp = json!({
            "candidates": [{
                "content": {
                    "parts": [{"text": "Hello there!"}],
                    "role": "model"
                },
                "finishReason": "STOP"
            }],
            "modelVersion": "gemini-1.5-pro",
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 5,
                "totalTokenCount": 15
            }
        });
        let data = serde_json::to_vec(&gemini_resp).unwrap();
        let result: Value =
            serde_json::from_str(&translate_non_stream("model", b"{}", &data).unwrap()).unwrap();

        assert_eq!(result["object"], "chat.completion");
        assert_eq!(result["model"], "gemini-1.5-pro");
        assert_eq!(result["choices"][0]["message"]["role"], "assistant");
        assert_eq!(result["choices"][0]["message"]["content"], "Hello there!");
        assert_eq!(result["choices"][0]["finish_reason"], "stop");
        assert_eq!(result["usage"]["prompt_tokens"], 10);
        assert_eq!(result["usage"]["completion_tokens"], 5);
        assert_eq!(result["usage"]["total_tokens"], 15);
    }

    #[test]
    fn test_non_stream_function_call() {
        let gemini_resp = json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "functionCall": {
                            "name": "get_weather",
                            "args": {"city": "SF"}
                        }
                    }],
                    "role": "model"
                },
                "finishReason": "STOP"
            }],
            "modelVersion": "gemini-1.5-pro"
        });
        let data = serde_json::to_vec(&gemini_resp).unwrap();
        let result: Value =
            serde_json::from_str(&translate_non_stream("model", b"{}", &data).unwrap()).unwrap();

        // content should be null when only tool calls
        assert_eq!(result["choices"][0]["message"]["content"], Value::Null);
        let tool_calls = result["choices"][0]["message"]["tool_calls"]
            .as_array()
            .unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0]["type"], "function");
        assert_eq!(tool_calls[0]["function"]["name"], "get_weather");
        let args: Value =
            serde_json::from_str(tool_calls[0]["function"]["arguments"].as_str().unwrap()).unwrap();
        assert_eq!(args, json!({"city": "SF"}));
    }

    #[test]
    fn test_non_stream_finish_reason_mapping() {
        let test_cases = vec![
            ("STOP", "stop"),
            ("MAX_TOKENS", "length"),
            ("SAFETY", "content_filter"),
            ("RECITATION", "content_filter"),
        ];
        for (gemini_reason, expected) in test_cases {
            let gemini_resp = json!({
                "candidates": [{
                    "content": {"parts": [{"text": "Hi"}], "role": "model"},
                    "finishReason": gemini_reason
                }],
                "modelVersion": "gemini"
            });
            let data = serde_json::to_vec(&gemini_resp).unwrap();
            let result: Value =
                serde_json::from_str(&translate_non_stream("model", b"{}", &data).unwrap())
                    .unwrap();
            assert_eq!(
                result["choices"][0]["finish_reason"], expected,
                "Gemini '{gemini_reason}' should map to '{expected}'"
            );
        }
    }

    #[test]
    fn test_non_stream_no_candidates() {
        let gemini_resp = json!({
            "modelVersion": "gemini"
        });
        let data = serde_json::to_vec(&gemini_resp).unwrap();
        let result: Value =
            serde_json::from_str(&translate_non_stream("model", b"{}", &data).unwrap()).unwrap();
        assert_eq!(result["choices"][0]["message"]["content"], "");
        assert_eq!(result["choices"][0]["finish_reason"], "stop");
    }

    #[test]
    fn test_non_stream_no_usage() {
        let gemini_resp = json!({
            "candidates": [{
                "content": {"parts": [{"text": "Hi"}], "role": "model"},
                "finishReason": "STOP"
            }],
            "modelVersion": "gemini"
        });
        let data = serde_json::to_vec(&gemini_resp).unwrap();
        let result: Value =
            serde_json::from_str(&translate_non_stream("model", b"{}", &data).unwrap()).unwrap();
        assert!(result.get("usage").is_none());
    }

    // ==================
    // Stream tests
    // ==================

    fn new_state() -> TranslateState {
        TranslateState::default()
    }

    fn parse_chunk(s: &str) -> Value {
        serde_json::from_str(s).unwrap()
    }

    #[test]
    fn test_stream_first_chunk_initializes_state() {
        let mut state = new_state();
        let resp = json!({
            "candidates": [{
                "content": {"parts": [{"text": "Hi"}], "role": "model"}
            }],
            "modelVersion": "gemini-1.5-pro"
        });
        let data = serde_json::to_vec(&resp).unwrap();
        let chunks = translate_stream("model", b"{}", None, &data, &mut state).unwrap();

        // First chunk should be role initialization
        assert!(chunks.len() >= 2); // role chunk + content chunk
        let role_chunk = parse_chunk(&chunks[0]);
        assert_eq!(role_chunk["choices"][0]["delta"]["role"], "assistant");
        assert!(!state.response_id.is_empty());

        // Second chunk should be content
        let content_chunk = parse_chunk(&chunks[1]);
        assert_eq!(content_chunk["choices"][0]["delta"]["content"], "Hi");
    }

    #[test]
    fn test_stream_text_content() {
        let mut state = new_state();
        state.response_id = "chatcmpl-test".to_string();
        state.created = 1000;
        state.model = "gemini".to_string();

        let resp = json!({
            "candidates": [{
                "content": {"parts": [{"text": "World"}], "role": "model"}
            }]
        });
        let data = serde_json::to_vec(&resp).unwrap();
        let chunks = translate_stream("model", b"{}", None, &data, &mut state).unwrap();

        assert_eq!(chunks.len(), 1);
        let chunk = parse_chunk(&chunks[0]);
        assert_eq!(chunk["choices"][0]["delta"]["content"], "World");
    }

    #[test]
    fn test_stream_function_call() {
        let mut state = new_state();
        state.response_id = "chatcmpl-test".to_string();
        state.created = 1000;
        state.model = "gemini".to_string();
        state.current_tool_call_index = -1;

        let resp = json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "functionCall": {
                            "name": "weather",
                            "args": {"city": "NYC"}
                        }
                    }],
                    "role": "model"
                }
            }]
        });
        let data = serde_json::to_vec(&resp).unwrap();
        let chunks = translate_stream("model", b"{}", None, &data, &mut state).unwrap();

        assert_eq!(chunks.len(), 1);
        let chunk = parse_chunk(&chunks[0]);
        let tc = &chunk["choices"][0]["delta"]["tool_calls"][0];
        assert_eq!(tc["function"]["name"], "weather");
        assert_eq!(state.current_tool_call_index, 0);
    }

    #[test]
    fn test_stream_finish_with_done() {
        let mut state = new_state();
        state.response_id = "chatcmpl-test".to_string();
        state.created = 1000;
        state.model = "gemini".to_string();

        let resp = json!({
            "candidates": [{
                "content": {"parts": [{"text": "Done"}], "role": "model"},
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 5
            }
        });
        let data = serde_json::to_vec(&resp).unwrap();
        let chunks = translate_stream("model", b"{}", None, &data, &mut state).unwrap();

        // Should have content chunk, finish chunk, and [DONE]
        assert!(chunks.len() >= 3);
        let last_json = chunks.iter().rev().nth(1).unwrap();
        let finish_chunk = parse_chunk(last_json);
        assert_eq!(finish_chunk["choices"][0]["finish_reason"], "stop");
        assert_eq!(finish_chunk["usage"]["prompt_tokens"], 10);
        assert_eq!(finish_chunk["usage"]["completion_tokens"], 5);

        assert_eq!(chunks.last().unwrap(), "[DONE]");
    }

    #[test]
    fn test_stream_model_version_update() {
        let mut state = new_state();
        state.response_id = "chatcmpl-test".to_string();
        state.created = 1000;
        state.model = "unknown".to_string();

        let resp = json!({
            "candidates": [{
                "content": {"parts": [{"text": "Hi"}], "role": "model"}
            }],
            "modelVersion": "gemini-1.5-flash"
        });
        let data = serde_json::to_vec(&resp).unwrap();
        translate_stream("model", b"{}", None, &data, &mut state).unwrap();

        assert_eq!(state.model, "gemini-1.5-flash");
    }

    #[test]
    fn test_stream_no_candidates() {
        let mut state = new_state();
        state.response_id = "chatcmpl-test".to_string();
        state.created = 1000;
        state.model = "gemini".to_string();

        let resp = json!({});
        let data = serde_json::to_vec(&resp).unwrap();
        let chunks = translate_stream("model", b"{}", None, &data, &mut state).unwrap();
        assert!(chunks.is_empty());
    }
}
