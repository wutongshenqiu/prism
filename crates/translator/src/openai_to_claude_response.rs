use crate::TranslateState;
use crate::common::map_openai_finish_reason_to_claude;
use prism_types::error::ProxyError;
use serde_json::{Value, json};

/// Translate an OpenAI Chat Completions non-streaming response to Claude Messages format.
pub fn translate_non_stream(
    _model: &str,
    _original_req: &[u8],
    data: &[u8],
) -> Result<String, ProxyError> {
    let resp: Value = serde_json::from_slice(data)?;

    let id = resp
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let model = resp
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let choice = resp
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|a| a.first());

    let mut content_blocks = Vec::new();

    if let Some(choice) = choice {
        let empty = json!({});
        let message = choice.get("message").unwrap_or(&empty);

        // Handle reasoning_content → thinking block
        if let Some(reasoning) = message.get("reasoning_content").and_then(|r| r.as_str())
            && !reasoning.is_empty()
        {
            content_blocks.push(json!({
                "type": "thinking",
                "thinking": reasoning,
                "signature": ""
            }));
        }

        // Handle text content
        if let Some(content) = message.get("content").and_then(|c| c.as_str())
            && !content.is_empty()
        {
            content_blocks.push(json!({"type": "text", "text": content}));
        }

        // Handle tool_calls → tool_use blocks
        if let Some(tool_calls) = message.get("tool_calls").and_then(|tc| tc.as_array()) {
            for tc in tool_calls {
                let tc_id = tc.get("id").and_then(|v| v.as_str()).unwrap_or("");
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
                let input: Value = serde_json::from_str(arguments_str).unwrap_or(json!({}));

                content_blocks.push(json!({
                    "type": "tool_use",
                    "id": tc_id,
                    "name": name,
                    "input": input,
                }));
            }
        }
    }

    if content_blocks.is_empty() {
        content_blocks.push(json!({"type": "text", "text": ""}));
    }

    let finish_reason = choice.and_then(|c| c.get("finish_reason").and_then(|f| f.as_str()));
    let stop_reason = map_openai_finish_reason_to_claude(finish_reason);

    let mut claude_resp = json!({
        "id": id,
        "type": "message",
        "role": "assistant",
        "model": model,
        "content": content_blocks,
        "stop_reason": stop_reason,
    });

    // Map usage
    if let Some(usage) = resp.get("usage") {
        let input_tokens = usage
            .get("prompt_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let output_tokens = usage
            .get("completion_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        claude_resp["usage"] = json!({
            "input_tokens": input_tokens,
            "output_tokens": output_tokens,
        });
    }

    serde_json::to_string(&claude_resp).map_err(|e| ProxyError::Translation(e.to_string()))
}

/// Translate an OpenAI Chat Completions streaming chunk to Claude Messages SSE events.
pub fn translate_stream(
    _model: &str,
    _original_req: &[u8],
    _event_type: Option<&str>,
    data: &[u8],
    state: &mut TranslateState,
) -> Result<Vec<String>, ProxyError> {
    let event: Value = serde_json::from_slice(data)?;
    let mut lines = Vec::new();

    let choices = event.get("choices").and_then(|c| c.as_array());
    let choice = choices.and_then(|a| a.first());

    if let Some(choice) = choice {
        let empty_delta = json!({});
        let delta = choice.get("delta").unwrap_or(&empty_delta);
        let finish_reason = choice.get("finish_reason").and_then(|f| f.as_str());

        // Handle role delta → message_start
        if delta.get("role").is_some() && !state.sent_role {
            let id = event
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let model = event
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            state.response_id = id.clone();
            state.model = model.clone();
            state.created = chrono::Utc::now().timestamp();
            state.sent_role = true;
            state.current_content_index = None;
            state.current_tool_call_index = None;
            state.input_tokens = 0;

            let msg_start = json!({
                "type": "message_start",
                "message": {
                    "id": id,
                    "type": "message",
                    "role": "assistant",
                    "model": model,
                    "content": [],
                    "stop_reason": null,
                    "stop_sequence": null,
                    "usage": {"input_tokens": 0, "output_tokens": 0}
                }
            });
            lines.push(format!(
                "event: message_start\ndata: {}",
                serde_json::to_string(&msg_start)?
            ));

            // Start first content block (text)
            let cb_start = json!({
                "type": "content_block_start",
                "index": 0,
                "content_block": {"type": "text", "text": ""}
            });
            state.current_content_index = Some(0);
            lines.push(format!(
                "event: content_block_start\ndata: {}",
                serde_json::to_string(&cb_start)?
            ));
        }

        // Handle reasoning_content delta → thinking_delta
        if let Some(reasoning) = delta.get("reasoning_content").and_then(|c| c.as_str()) {
            let delta_event = json!({
                "type": "content_block_delta",
                "index": state.current_content_index.unwrap_or(0),
                "delta": {"type": "thinking_delta", "thinking": reasoning}
            });
            lines.push(format!(
                "event: content_block_delta\ndata: {}",
                serde_json::to_string(&delta_event)?
            ));
        }

        // Handle text content delta
        if let Some(text) = delta.get("content").and_then(|c| c.as_str()) {
            let delta_event = json!({
                "type": "content_block_delta",
                "index": state.current_content_index.unwrap_or(0),
                "delta": {"type": "text_delta", "text": text}
            });
            lines.push(format!(
                "event: content_block_delta\ndata: {}",
                serde_json::to_string(&delta_event)?
            ));
        }

        // Handle tool_calls delta
        if let Some(tool_calls) = delta.get("tool_calls").and_then(|tc| tc.as_array()) {
            for tc in tool_calls {
                if let Some(func) = tc.get("function") {
                    // New tool call (has name)
                    if let Some(name) = func.get("name").and_then(|n| n.as_str()) {
                        // Close previous content block if needed
                        if let Some(idx) = state.current_content_index {
                            let cb_stop = json!({
                                "type": "content_block_stop",
                                "index": idx
                            });
                            lines.push(format!(
                                "event: content_block_stop\ndata: {}",
                                serde_json::to_string(&cb_stop)?
                            ));
                        }

                        let new_idx = state.next_content_index() as u32;
                        let tc_id = tc
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();

                        let cb_start = json!({
                            "type": "content_block_start",
                            "index": new_idx,
                            "content_block": {
                                "type": "tool_use",
                                "id": tc_id,
                                "name": name,
                                "input": {}
                            }
                        });
                        lines.push(format!(
                            "event: content_block_start\ndata: {}",
                            serde_json::to_string(&cb_start)?
                        ));
                    }

                    // Tool arguments delta
                    if let Some(args) = func.get("arguments").and_then(|a| a.as_str())
                        && !args.is_empty()
                    {
                        let delta_event = json!({
                            "type": "content_block_delta",
                            "index": state.current_content_index.unwrap_or(0),
                            "delta": {"type": "input_json_delta", "partial_json": args}
                        });
                        lines.push(format!(
                            "event: content_block_delta\ndata: {}",
                            serde_json::to_string(&delta_event)?
                        ));
                    }
                }
            }
        }

        // Handle finish
        if let Some(reason) = finish_reason {
            // Close current content block
            if let Some(idx) = state.current_content_index {
                let cb_stop = json!({"type": "content_block_stop", "index": idx});
                lines.push(format!(
                    "event: content_block_stop\ndata: {}",
                    serde_json::to_string(&cb_stop)?
                ));
            }

            let stop_reason = map_openai_finish_reason_to_claude(Some(reason));
            let mut usage_output = 0u64;

            if let Some(usage) = event.get("usage") {
                state.input_tokens = usage
                    .get("prompt_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                usage_output = usage
                    .get("completion_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
            }

            let msg_delta = json!({
                "type": "message_delta",
                "delta": {"stop_reason": stop_reason},
                "usage": {"output_tokens": usage_output}
            });
            lines.push(format!(
                "event: message_delta\ndata: {}",
                serde_json::to_string(&msg_delta)?
            ));

            let msg_stop = json!({"type": "message_stop"});
            lines.push(format!(
                "event: message_stop\ndata: {}",
                serde_json::to_string(&msg_stop)?
            ));
        }
    }

    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Non-stream tests ──

    #[test]
    fn test_non_stream_basic_text() {
        let openai_resp = json!({
            "id": "chatcmpl-123",
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "Hello!"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        });
        let data = serde_json::to_vec(&openai_resp).unwrap();
        let result: Value =
            serde_json::from_str(&translate_non_stream("model", b"{}", &data).unwrap()).unwrap();

        assert_eq!(result["id"], "chatcmpl-123");
        assert_eq!(result["type"], "message");
        assert_eq!(result["role"], "assistant");
        assert_eq!(result["model"], "gpt-4o");
        assert_eq!(result["content"][0]["type"], "text");
        assert_eq!(result["content"][0]["text"], "Hello!");
        assert_eq!(result["stop_reason"], "end_turn");
        assert_eq!(result["usage"]["input_tokens"], 10);
        assert_eq!(result["usage"]["output_tokens"], 5);
    }

    #[test]
    fn test_non_stream_tool_calls() {
        let openai_resp = json!({
            "id": "chatcmpl-456",
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_123",
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

        assert_eq!(result["stop_reason"], "tool_use");
        let content = result["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "tool_use");
        assert_eq!(content[0]["id"], "call_123");
        assert_eq!(content[0]["name"], "get_weather");
        assert_eq!(content[0]["input"]["city"], "SF");
    }

    #[test]
    fn test_non_stream_stop_reason_mapping() {
        let test_cases = vec![
            ("stop", "end_turn"),
            ("length", "max_tokens"),
            ("tool_calls", "tool_use"),
        ];
        for (openai_reason, expected_reason) in test_cases {
            let openai_resp = json!({
                "id": "chatcmpl-sr",
                "model": "gpt-4o",
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
                result["stop_reason"], expected_reason,
                "OpenAI '{openai_reason}' should map to '{expected_reason}'"
            );
        }
    }

    #[test]
    fn test_non_stream_with_reasoning() {
        let openai_resp = json!({
            "id": "chatcmpl-reason",
            "model": "gpt-4o",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "reasoning_content": "Step by step...",
                    "content": "The answer is 42."
                },
                "finish_reason": "stop"
            }]
        });
        let data = serde_json::to_vec(&openai_resp).unwrap();
        let result: Value =
            serde_json::from_str(&translate_non_stream("model", b"{}", &data).unwrap()).unwrap();

        let content = result["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "thinking");
        assert_eq!(content[0]["thinking"], "Step by step...");
        assert_eq!(content[1]["type"], "text");
        assert_eq!(content[1]["text"], "The answer is 42.");
    }

    // ── Stream tests ──

    fn new_state() -> TranslateState {
        TranslateState::default()
    }

    #[test]
    fn test_stream_role_delta() {
        let mut state = new_state();
        let chunk = json!({
            "id": "chatcmpl-stream",
            "model": "gpt-4o",
            "choices": [{"index": 0, "delta": {"role": "assistant", "content": ""}, "finish_reason": null}]
        });
        let data = serde_json::to_vec(&chunk).unwrap();
        let lines = translate_stream("model", b"{}", None, &data, &mut state).unwrap();

        // Should produce message_start + content_block_start
        assert!(lines.len() >= 2);
        assert!(lines[0].starts_with("event: message_start"));
        assert!(lines[1].starts_with("event: content_block_start"));
        assert!(state.sent_role);
    }

    #[test]
    fn test_stream_text_delta() {
        let mut state = new_state();
        state.sent_role = true;
        state.current_content_index = Some(0);

        let chunk = json!({
            "id": "chatcmpl-stream",
            "model": "gpt-4o",
            "choices": [{"index": 0, "delta": {"content": "Hello"}, "finish_reason": null}]
        });
        let data = serde_json::to_vec(&chunk).unwrap();
        let lines = translate_stream("model", b"{}", None, &data, &mut state).unwrap();

        assert_eq!(lines.len(), 1);
        assert!(lines[0].starts_with("event: content_block_delta"));
        assert!(lines[0].contains("text_delta"));
        assert!(lines[0].contains("Hello"));
    }

    #[test]
    fn test_stream_finish() {
        let mut state = new_state();
        state.sent_role = true;
        state.current_content_index = Some(0);

        let chunk = json!({
            "id": "chatcmpl-stream",
            "model": "gpt-4o",
            "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        });
        let data = serde_json::to_vec(&chunk).unwrap();
        let lines = translate_stream("model", b"{}", None, &data, &mut state).unwrap();

        // Should produce content_block_stop + message_delta + message_stop
        assert!(lines.len() >= 3);
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("event: content_block_stop"))
        );
        assert!(lines.iter().any(|l| l.starts_with("event: message_delta")));
        assert!(lines.iter().any(|l| l.starts_with("event: message_stop")));
    }

    #[test]
    fn test_stream_tool_call() {
        let mut state = new_state();
        state.sent_role = true;
        state.current_content_index = Some(0);

        // Tool call start
        let chunk = json!({
            "id": "chatcmpl-stream",
            "model": "gpt-4o",
            "choices": [{"index": 0, "delta": {
                "tool_calls": [{"index": 0, "id": "call_1", "type": "function", "function": {"name": "get_weather", "arguments": ""}}]
            }, "finish_reason": null}]
        });
        let data = serde_json::to_vec(&chunk).unwrap();
        let lines = translate_stream("model", b"{}", None, &data, &mut state).unwrap();

        // Should have content_block_stop (for text) + content_block_start (for tool)
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("event: content_block_stop"))
        );
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("event: content_block_start") && l.contains("tool_use"))
        );
    }
}
