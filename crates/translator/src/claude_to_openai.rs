use crate::TranslateState;
use crate::common::{
    build_assistant_message, build_openai_chunk, build_openai_response, build_tool_call,
    build_tool_call_delta, map_claude_finish_reason,
};
use prism_core::error::ProxyError;
use serde_json::{Value, json};

pub fn translate_non_stream(
    _model: &str,
    _original_req: &[u8],
    data: &[u8],
) -> Result<String, ProxyError> {
    let resp: Value = serde_json::from_slice(data)?;

    let id = format!(
        "chatcmpl-{}",
        resp.get("id").and_then(|v| v.as_str()).unwrap_or("unknown")
    );
    let model = resp
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let created = chrono::Utc::now().timestamp();

    // Extract text content and tool_use blocks
    let mut text_parts = Vec::new();
    let mut tool_calls = Vec::new();
    let mut tool_call_index = 0u32;

    if let Some(content) = resp.get("content").and_then(|c| c.as_array()) {
        for block in content {
            let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
            match block_type {
                "text" => {
                    if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                        text_parts.push(text.to_string());
                    }
                }
                "tool_use" => {
                    let tc_id = block.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    let name = block.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let input = block.get("input").cloned().unwrap_or(json!({}));
                    let arguments = serde_json::to_string(&input).unwrap_or_default();

                    tool_calls.push(build_tool_call(tc_id, name, &arguments, tool_call_index));
                    tool_call_index += 1;
                }
                _ => {}
            }
        }
    }

    let finish_reason = map_claude_finish_reason(resp.get("stop_reason").and_then(|v| v.as_str()));

    let content_str = text_parts.join("");
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
    let usage = if let Some(u) = resp.get("usage") {
        let input_tokens = u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
        let output_tokens = u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
        Some(json!({
            "prompt_tokens": input_tokens,
            "completion_tokens": output_tokens,
            "total_tokens": input_tokens + output_tokens,
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
    event_type: Option<&str>,
    data: &[u8],
    state: &mut TranslateState,
) -> Result<Vec<String>, ProxyError> {
    let event: Value = serde_json::from_slice(data)?;
    let mut chunks = Vec::new();

    match event_type {
        Some("message_start") => {
            if let Some(msg) = event.get("message") {
                state.response_id = format!(
                    "chatcmpl-{}",
                    msg.get("id").and_then(|v| v.as_str()).unwrap_or("unknown")
                );
                state.model = msg
                    .get("model")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                state.created = chrono::Utc::now().timestamp();
                state.current_content_index = -1;
                state.current_tool_call_index = -1;
                state.sent_role = false;
                state.input_tokens = msg
                    .get("usage")
                    .and_then(|u| u.get("input_tokens"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
            }

            // Emit initial chunk with role
            let chunk = build_openai_chunk(
                &state.response_id,
                state.created,
                &state.model,
                json!({"role": "assistant", "content": ""}),
                None,
            );
            state.sent_role = true;
            chunks.push(serde_json::to_string(&chunk)?);
        }

        Some("content_block_start") => {
            state.current_content_index += 1;

            if let Some(cb) = event.get("content_block") {
                let block_type = cb.get("type").and_then(|t| t.as_str()).unwrap_or("");
                if block_type == "tool_use" {
                    state.current_tool_call_index += 1;
                    let tc_id = cb.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    let name = cb.get("name").and_then(|v| v.as_str()).unwrap_or("");

                    let delta = json!({
                        "tool_calls": [build_tool_call_delta(
                            state.current_tool_call_index,
                            tc_id,
                            name,
                            "",
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

        Some("content_block_delta") => {
            if let Some(delta) = event.get("delta") {
                let delta_type = delta.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match delta_type {
                    "text_delta" => {
                        let text = delta.get("text").and_then(|t| t.as_str()).unwrap_or("");
                        let chunk = build_openai_chunk(
                            &state.response_id,
                            state.created,
                            &state.model,
                            json!({"content": text}),
                            None,
                        );
                        chunks.push(serde_json::to_string(&chunk)?);
                    }
                    "input_json_delta" => {
                        let partial = delta
                            .get("partial_json")
                            .and_then(|t| t.as_str())
                            .unwrap_or("");
                        let chunk = build_openai_chunk(
                            &state.response_id,
                            state.created,
                            &state.model,
                            json!({
                                "tool_calls": [{
                                    "index": state.current_tool_call_index,
                                    "function": {
                                        "arguments": partial,
                                    },
                                }],
                            }),
                            None,
                        );
                        chunks.push(serde_json::to_string(&chunk)?);
                    }
                    _ => {}
                }
            }
        }

        Some("message_delta") => {
            if let Some(delta) = event.get("delta") {
                let finish_reason =
                    map_claude_finish_reason(delta.get("stop_reason").and_then(|v| v.as_str()));

                let mut chunk = build_openai_chunk(
                    &state.response_id,
                    state.created,
                    &state.model,
                    json!({}),
                    Some(finish_reason),
                );

                // Include usage if available
                if let Some(usage) = event.get("usage") {
                    let output_tokens = usage
                        .get("output_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    let input_tokens = state.input_tokens;
                    chunk["usage"] = json!({
                        "prompt_tokens": input_tokens,
                        "completion_tokens": output_tokens,
                        "total_tokens": input_tokens + output_tokens,
                    });
                }

                chunks.push(serde_json::to_string(&chunk)?);
            }
        }

        Some("message_stop") => {
            chunks.push("[DONE]".to_string());
        }

        _ => {
            // ping, content_block_stop, etc. - skip
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
        let claude_resp = json!({
            "id": "msg_abc123",
            "model": "claude-3-5-sonnet-20241022",
            "content": [{"type": "text", "text": "Hello there!"}],
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 10, "output_tokens": 5}
        });
        let data = serde_json::to_vec(&claude_resp).unwrap();
        let result: Value =
            serde_json::from_str(&translate_non_stream("model", b"{}", &data).unwrap()).unwrap();

        assert_eq!(result["id"], "chatcmpl-msg_abc123");
        assert_eq!(result["object"], "chat.completion");
        assert_eq!(result["model"], "claude-3-5-sonnet-20241022");
        assert_eq!(result["choices"][0]["message"]["role"], "assistant");
        assert_eq!(result["choices"][0]["message"]["content"], "Hello there!");
        assert_eq!(result["choices"][0]["finish_reason"], "stop");
        assert_eq!(result["usage"]["prompt_tokens"], 10);
        assert_eq!(result["usage"]["completion_tokens"], 5);
        assert_eq!(result["usage"]["total_tokens"], 15);
    }

    #[test]
    fn test_non_stream_tool_use() {
        let claude_resp = json!({
            "id": "msg_tool",
            "model": "claude-3-5-sonnet-20241022",
            "content": [{
                "type": "tool_use",
                "id": "toolu_123",
                "name": "get_weather",
                "input": {"city": "SF"}
            }],
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 20, "output_tokens": 30}
        });
        let data = serde_json::to_vec(&claude_resp).unwrap();
        let result: Value =
            serde_json::from_str(&translate_non_stream("model", b"{}", &data).unwrap()).unwrap();

        assert_eq!(result["choices"][0]["finish_reason"], "tool_calls");
        // content should be null when only tool calls
        assert_eq!(result["choices"][0]["message"]["content"], Value::Null);
        let tool_calls = result["choices"][0]["message"]["tool_calls"]
            .as_array()
            .unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0]["id"], "toolu_123");
        assert_eq!(tool_calls[0]["type"], "function");
        assert_eq!(tool_calls[0]["function"]["name"], "get_weather");
        // arguments should be a JSON string
        let args: Value =
            serde_json::from_str(tool_calls[0]["function"]["arguments"].as_str().unwrap()).unwrap();
        assert_eq!(args, json!({"city": "SF"}));
    }

    #[test]
    fn test_non_stream_mixed_text_and_tool() {
        let claude_resp = json!({
            "id": "msg_mix",
            "model": "claude-3-5-sonnet-20241022",
            "content": [
                {"type": "text", "text": "Let me check the weather."},
                {"type": "tool_use", "id": "toolu_456", "name": "weather", "input": {}}
            ],
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 5, "output_tokens": 10}
        });
        let data = serde_json::to_vec(&claude_resp).unwrap();
        let result: Value =
            serde_json::from_str(&translate_non_stream("model", b"{}", &data).unwrap()).unwrap();

        assert_eq!(
            result["choices"][0]["message"]["content"],
            "Let me check the weather."
        );
        assert_eq!(
            result["choices"][0]["message"]["tool_calls"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(result["choices"][0]["finish_reason"], "tool_calls");
    }

    #[test]
    fn test_non_stream_stop_reason_mapping() {
        let test_cases = vec![
            ("end_turn", "stop"),
            ("max_tokens", "length"),
            ("tool_use", "tool_calls"),
            ("stop_sequence", "stop"),
        ];
        for (claude_reason, expected_reason) in test_cases {
            let claude_resp = json!({
                "id": "msg_sr",
                "model": "claude-3-5-sonnet-20241022",
                "content": [{"type": "text", "text": "Hi"}],
                "stop_reason": claude_reason
            });
            let data = serde_json::to_vec(&claude_resp).unwrap();
            let result: Value =
                serde_json::from_str(&translate_non_stream("model", b"{}", &data).unwrap())
                    .unwrap();
            assert_eq!(
                result["choices"][0]["finish_reason"], expected_reason,
                "Claude stop_reason '{claude_reason}' should map to '{expected_reason}'"
            );
        }
    }

    #[test]
    fn test_non_stream_no_usage() {
        let claude_resp = json!({
            "id": "msg_nu",
            "model": "claude-3-5-sonnet-20241022",
            "content": [{"type": "text", "text": "Hi"}],
            "stop_reason": "end_turn"
        });
        let data = serde_json::to_vec(&claude_resp).unwrap();
        let result: Value =
            serde_json::from_str(&translate_non_stream("model", b"{}", &data).unwrap()).unwrap();
        assert!(result.get("usage").is_none());
    }

    #[test]
    fn test_non_stream_empty_content() {
        let claude_resp = json!({
            "id": "msg_ec",
            "model": "claude-3-5-sonnet-20241022",
            "content": [],
            "stop_reason": "end_turn"
        });
        let data = serde_json::to_vec(&claude_resp).unwrap();
        let result: Value =
            serde_json::from_str(&translate_non_stream("model", b"{}", &data).unwrap()).unwrap();
        assert_eq!(result["choices"][0]["message"]["content"], "");
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
    fn test_stream_message_start() {
        let mut state = new_state();
        let event = json!({
            "type": "message_start",
            "message": {
                "id": "msg_stream_1",
                "model": "claude-3-5-sonnet-20241022",
                "usage": {"input_tokens": 15}
            }
        });
        let data = serde_json::to_vec(&event).unwrap();
        let chunks =
            translate_stream("model", b"{}", Some("message_start"), &data, &mut state).unwrap();

        assert_eq!(chunks.len(), 1);
        let chunk = parse_chunk(&chunks[0]);
        assert_eq!(chunk["id"], "chatcmpl-msg_stream_1");
        assert_eq!(chunk["object"], "chat.completion.chunk");
        assert_eq!(chunk["choices"][0]["delta"]["role"], "assistant");
        assert!(state.sent_role);
        assert_eq!(state.input_tokens, 15);
    }

    #[test]
    fn test_stream_content_block_start_tool_use() {
        let mut state = new_state();
        state.response_id = "chatcmpl-test".to_string();
        state.created = 1000;
        state.model = "claude-3-5-sonnet-20241022".to_string();
        // Simulate post-message_start state
        state.current_content_index = -1;
        state.current_tool_call_index = -1;

        let event = json!({
            "type": "content_block_start",
            "index": 0,
            "content_block": {
                "type": "tool_use",
                "id": "toolu_abc",
                "name": "get_weather"
            }
        });
        let data = serde_json::to_vec(&event).unwrap();
        let chunks = translate_stream(
            "model",
            b"{}",
            Some("content_block_start"),
            &data,
            &mut state,
        )
        .unwrap();

        assert_eq!(chunks.len(), 1);
        let chunk = parse_chunk(&chunks[0]);
        assert_eq!(
            chunk["choices"][0]["delta"]["tool_calls"][0]["id"],
            "toolu_abc"
        );
        assert_eq!(
            chunk["choices"][0]["delta"]["tool_calls"][0]["function"]["name"],
            "get_weather"
        );
        assert_eq!(state.current_tool_call_index, 0);
    }

    #[test]
    fn test_stream_content_block_start_text() {
        let mut state = new_state();
        state.response_id = "chatcmpl-test".to_string();
        // Simulate post-message_start state
        state.current_content_index = -1;

        let event = json!({
            "type": "content_block_start",
            "index": 0,
            "content_block": {"type": "text", "text": ""}
        });
        let data = serde_json::to_vec(&event).unwrap();
        let chunks = translate_stream(
            "model",
            b"{}",
            Some("content_block_start"),
            &data,
            &mut state,
        )
        .unwrap();

        // Text block start should not emit a chunk
        assert!(chunks.is_empty());
        assert_eq!(state.current_content_index, 0);
    }

    #[test]
    fn test_stream_text_delta() {
        let mut state = new_state();
        state.response_id = "chatcmpl-test".to_string();
        state.created = 1000;
        state.model = "claude".to_string();

        let event = json!({
            "type": "content_block_delta",
            "delta": {"type": "text_delta", "text": "Hello"}
        });
        let data = serde_json::to_vec(&event).unwrap();
        let chunks = translate_stream(
            "model",
            b"{}",
            Some("content_block_delta"),
            &data,
            &mut state,
        )
        .unwrap();

        assert_eq!(chunks.len(), 1);
        let chunk = parse_chunk(&chunks[0]);
        assert_eq!(chunk["choices"][0]["delta"]["content"], "Hello");
        assert_eq!(chunk["choices"][0]["finish_reason"], Value::Null);
    }

    #[test]
    fn test_stream_input_json_delta() {
        let mut state = new_state();
        state.response_id = "chatcmpl-test".to_string();
        state.created = 1000;
        state.model = "claude".to_string();
        state.current_tool_call_index = 0;

        let event = json!({
            "type": "content_block_delta",
            "delta": {"type": "input_json_delta", "partial_json": "{\"city\":"}
        });
        let data = serde_json::to_vec(&event).unwrap();
        let chunks = translate_stream(
            "model",
            b"{}",
            Some("content_block_delta"),
            &data,
            &mut state,
        )
        .unwrap();

        assert_eq!(chunks.len(), 1);
        let chunk = parse_chunk(&chunks[0]);
        assert_eq!(
            chunk["choices"][0]["delta"]["tool_calls"][0]["function"]["arguments"],
            "{\"city\":"
        );
        assert_eq!(chunk["choices"][0]["delta"]["tool_calls"][0]["index"], 0);
    }

    #[test]
    fn test_stream_message_delta_end_turn() {
        let mut state = new_state();
        state.response_id = "chatcmpl-test".to_string();
        state.created = 1000;
        state.model = "claude".to_string();
        state.input_tokens = 10;

        let event = json!({
            "type": "message_delta",
            "delta": {"stop_reason": "end_turn"},
            "usage": {"output_tokens": 20}
        });
        let data = serde_json::to_vec(&event).unwrap();
        let chunks =
            translate_stream("model", b"{}", Some("message_delta"), &data, &mut state).unwrap();

        assert_eq!(chunks.len(), 1);
        let chunk = parse_chunk(&chunks[0]);
        assert_eq!(chunk["choices"][0]["finish_reason"], "stop");
        assert_eq!(chunk["usage"]["prompt_tokens"], 10);
        assert_eq!(chunk["usage"]["completion_tokens"], 20);
        assert_eq!(chunk["usage"]["total_tokens"], 30);
    }

    #[test]
    fn test_stream_message_delta_tool_use() {
        let mut state = new_state();
        state.response_id = "chatcmpl-test".to_string();
        state.created = 1000;
        state.model = "claude".to_string();

        let event = json!({
            "type": "message_delta",
            "delta": {"stop_reason": "tool_use"}
        });
        let data = serde_json::to_vec(&event).unwrap();
        let chunks =
            translate_stream("model", b"{}", Some("message_delta"), &data, &mut state).unwrap();

        let chunk = parse_chunk(&chunks[0]);
        assert_eq!(chunk["choices"][0]["finish_reason"], "tool_calls");
    }

    #[test]
    fn test_stream_message_stop() {
        let mut state = new_state();
        let event = json!({"type": "message_stop"});
        let data = serde_json::to_vec(&event).unwrap();
        let chunks =
            translate_stream("model", b"{}", Some("message_stop"), &data, &mut state).unwrap();

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "[DONE]");
    }

    #[test]
    fn test_stream_ping_skipped() {
        let mut state = new_state();
        let event = json!({"type": "ping"});
        let data = serde_json::to_vec(&event).unwrap();
        let chunks = translate_stream("model", b"{}", Some("ping"), &data, &mut state).unwrap();
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_stream_content_block_stop_skipped() {
        let mut state = new_state();
        let event = json!({"type": "content_block_stop", "index": 0});
        let data = serde_json::to_vec(&event).unwrap();
        let chunks = translate_stream(
            "model",
            b"{}",
            Some("content_block_stop"),
            &data,
            &mut state,
        )
        .unwrap();
        assert!(chunks.is_empty());
    }
}
