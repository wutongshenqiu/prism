use serde_json::{Value, json};

/// Map Claude stop_reason to OpenAI finish_reason.
pub fn map_claude_finish_reason(reason: Option<&str>) -> &'static str {
    match reason {
        Some("end_turn") => "stop",
        Some("max_tokens") => "length",
        Some("tool_use") => "tool_calls",
        Some("stop_sequence") => "stop",
        _ => "stop",
    }
}

/// Map OpenAI finish_reason to Claude stop_reason.
pub fn map_openai_finish_reason_to_claude(reason: Option<&str>) -> &'static str {
    match reason {
        Some("stop") => "end_turn",
        Some("length") => "max_tokens",
        Some("tool_calls") => "tool_use",
        Some("content_filter") => "end_turn",
        _ => "end_turn",
    }
}

/// Map Gemini finishReason to OpenAI finish_reason.
pub fn map_gemini_finish_reason(reason: Option<&str>) -> &'static str {
    match reason {
        Some("STOP") => "stop",
        Some("MAX_TOKENS") => "length",
        Some("SAFETY") => "content_filter",
        Some("RECITATION") => "content_filter",
        _ => "stop",
    }
}

/// Build an OpenAI streaming chunk wrapper.
pub fn build_openai_chunk(
    response_id: &str,
    created: i64,
    model: &str,
    delta: Value,
    finish_reason: Option<&str>,
) -> Value {
    json!({
        "id": response_id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": delta,
            "finish_reason": finish_reason,
        }],
    })
}

/// Build a complete OpenAI non-stream response.
pub fn build_openai_response(
    id: &str,
    created: i64,
    model: &str,
    message: Value,
    finish_reason: &str,
    usage: Option<Value>,
) -> Value {
    let mut resp = json!({
        "id": id,
        "object": "chat.completion",
        "created": created,
        "model": model,
        "choices": [{
            "index": 0,
            "message": message,
            "finish_reason": finish_reason,
        }],
    });
    if let Some(usage) = usage {
        resp["usage"] = usage;
    }
    resp
}

/// Build a tool call object for non-stream responses.
pub fn build_tool_call(id: &str, name: &str, arguments: &str, index: u32) -> Value {
    json!({
        "id": id,
        "type": "function",
        "function": {
            "name": name,
            "arguments": arguments,
        },
        "index": index,
    })
}

/// Build a tool call delta for streaming (initial tool_call with name).
pub fn build_tool_call_delta(index: i32, id: &str, name: &str, arguments: &str) -> Value {
    json!({
        "index": index,
        "id": id,
        "type": "function",
        "function": {
            "name": name,
            "arguments": arguments,
        },
    })
}

/// Build an assistant message with optional text content and optional tool_calls.
pub fn build_assistant_message(content: Option<&str>, tool_calls: Option<Vec<Value>>) -> Value {
    let content_val = match (content, &tool_calls) {
        (Some(c), _) if !c.is_empty() => Value::String(c.to_string()),
        (_, Some(tc)) if !tc.is_empty() => Value::Null,
        _ => Value::String(String::new()),
    };

    let mut message = json!({
        "role": "assistant",
        "content": content_val,
    });

    if let Some(tool_calls) = tool_calls
        && !tool_calls.is_empty()
    {
        message["tool_calls"] = Value::Array(tool_calls);
    }

    message
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_claude_finish_reason() {
        assert_eq!(map_claude_finish_reason(Some("end_turn")), "stop");
        assert_eq!(map_claude_finish_reason(Some("max_tokens")), "length");
        assert_eq!(map_claude_finish_reason(Some("tool_use")), "tool_calls");
        assert_eq!(map_claude_finish_reason(Some("stop_sequence")), "stop");
        assert_eq!(map_claude_finish_reason(None), "stop");
        assert_eq!(map_claude_finish_reason(Some("unknown")), "stop");
    }

    #[test]
    fn test_map_openai_finish_reason_to_claude() {
        assert_eq!(map_openai_finish_reason_to_claude(Some("stop")), "end_turn");
        assert_eq!(
            map_openai_finish_reason_to_claude(Some("length")),
            "max_tokens"
        );
        assert_eq!(
            map_openai_finish_reason_to_claude(Some("tool_calls")),
            "tool_use"
        );
        assert_eq!(
            map_openai_finish_reason_to_claude(Some("content_filter")),
            "end_turn"
        );
        assert_eq!(map_openai_finish_reason_to_claude(None), "end_turn");
    }

    #[test]
    fn test_map_gemini_finish_reason() {
        assert_eq!(map_gemini_finish_reason(Some("STOP")), "stop");
        assert_eq!(map_gemini_finish_reason(Some("MAX_TOKENS")), "length");
        assert_eq!(map_gemini_finish_reason(Some("SAFETY")), "content_filter");
        assert_eq!(
            map_gemini_finish_reason(Some("RECITATION")),
            "content_filter"
        );
        assert_eq!(map_gemini_finish_reason(None), "stop");
    }

    #[test]
    fn test_build_openai_chunk() {
        let chunk = build_openai_chunk("id-1", 1000, "gpt-4", json!({"content": "hi"}), None);
        assert_eq!(chunk["id"], "id-1");
        assert_eq!(chunk["object"], "chat.completion.chunk");
        assert_eq!(chunk["created"], 1000);
        assert_eq!(chunk["model"], "gpt-4");
        assert_eq!(chunk["choices"][0]["delta"]["content"], "hi");
        assert_eq!(chunk["choices"][0]["finish_reason"], Value::Null);
    }

    #[test]
    fn test_build_openai_chunk_with_finish() {
        let chunk = build_openai_chunk("id-1", 1000, "gpt-4", json!({}), Some("stop"));
        assert_eq!(chunk["choices"][0]["finish_reason"], "stop");
    }

    #[test]
    fn test_build_openai_response() {
        let msg = json!({"role": "assistant", "content": "Hello"});
        let usage = Some(json!({"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}));
        let resp = build_openai_response("id-1", 1000, "gpt-4", msg, "stop", usage);

        assert_eq!(resp["id"], "id-1");
        assert_eq!(resp["object"], "chat.completion");
        assert_eq!(resp["choices"][0]["finish_reason"], "stop");
        assert_eq!(resp["usage"]["prompt_tokens"], 10);
    }

    #[test]
    fn test_build_openai_response_no_usage() {
        let msg = json!({"role": "assistant", "content": "Hi"});
        let resp = build_openai_response("id-1", 1000, "gpt-4", msg, "stop", None);
        assert!(resp.get("usage").is_none());
    }

    #[test]
    fn test_build_tool_call() {
        let tc = build_tool_call("call-1", "get_weather", "{\"city\":\"SF\"}", 0);
        assert_eq!(tc["id"], "call-1");
        assert_eq!(tc["type"], "function");
        assert_eq!(tc["function"]["name"], "get_weather");
        assert_eq!(tc["function"]["arguments"], "{\"city\":\"SF\"}");
        assert_eq!(tc["index"], 0);
    }

    #[test]
    fn test_build_tool_call_delta() {
        let delta = build_tool_call_delta(0, "call-1", "weather", "");
        assert_eq!(delta["index"], 0);
        assert_eq!(delta["id"], "call-1");
        assert_eq!(delta["type"], "function");
        assert_eq!(delta["function"]["name"], "weather");
    }

    #[test]
    fn test_build_assistant_message_text_only() {
        let msg = build_assistant_message(Some("Hello"), None);
        assert_eq!(msg["role"], "assistant");
        assert_eq!(msg["content"], "Hello");
        assert!(msg.get("tool_calls").is_none());
    }

    #[test]
    fn test_build_assistant_message_tool_only() {
        let tc = vec![build_tool_call("id", "fn", "{}", 0)];
        let msg = build_assistant_message(None, Some(tc));
        assert_eq!(msg["content"], Value::Null);
        assert_eq!(msg["tool_calls"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_build_assistant_message_mixed() {
        let tc = vec![build_tool_call("id", "fn", "{}", 0)];
        let msg = build_assistant_message(Some("Let me check"), Some(tc));
        assert_eq!(msg["content"], "Let me check");
        assert_eq!(msg["tool_calls"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_build_assistant_message_empty() {
        let msg = build_assistant_message(None, None);
        assert_eq!(msg["content"], "");
        assert!(msg.get("tool_calls").is_none());
    }
}
