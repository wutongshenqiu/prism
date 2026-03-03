use assert_json_diff::assert_json_eq;
/// Roundtrip integration tests: verify that translating a request to a target format
/// and then translating a corresponding response back produces valid OpenAI-format output.
use prism_core::provider::Format;
use prism_translator::{TranslateState, build_registry};
use serde_json::{Value, json};

#[test]
fn test_roundtrip_openai_to_claude_text() {
    let reg = build_registry();

    // 1. Translate OpenAI request → Claude format
    let openai_req = json!({
        "model": "gpt-4",
        "messages": [
            {"role": "system", "content": "You are helpful."},
            {"role": "user", "content": "What is 2+2?"}
        ],
        "max_tokens": 100
    });
    let raw = serde_json::to_vec(&openai_req).unwrap();
    let claude_req = reg
        .translate_request(
            Format::OpenAI,
            Format::Claude,
            "claude-3-5-sonnet-20241022",
            &raw,
            false,
        )
        .unwrap();
    let claude_req_val: Value = serde_json::from_slice(&claude_req).unwrap();

    // Verify Claude request structure
    assert_eq!(claude_req_val["model"], "claude-3-5-sonnet-20241022");
    assert_eq!(claude_req_val["system"], "You are helpful.");
    assert_eq!(claude_req_val["messages"][0]["role"], "user");
    assert_eq!(claude_req_val["max_tokens"], 100);

    // 2. Simulate Claude response
    let claude_resp = json!({
        "id": "msg_roundtrip",
        "model": "claude-3-5-sonnet-20241022",
        "content": [{"type": "text", "text": "2+2 equals 4."}],
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 15, "output_tokens": 8}
    });
    let resp_data = serde_json::to_vec(&claude_resp).unwrap();

    // 3. Translate Claude response → OpenAI format
    let openai_resp = reg
        .translate_non_stream(
            Format::OpenAI,
            Format::Claude,
            "claude-3-5-sonnet-20241022",
            &raw,
            &resp_data,
        )
        .unwrap();
    let result: Value = serde_json::from_str(&openai_resp).unwrap();

    // 4. Verify OpenAI response structure
    assert_eq!(result["object"], "chat.completion");
    assert_eq!(result["choices"][0]["message"]["role"], "assistant");
    assert_eq!(result["choices"][0]["message"]["content"], "2+2 equals 4.");
    assert_eq!(result["choices"][0]["finish_reason"], "stop");
    assert_eq!(result["usage"]["prompt_tokens"], 15);
    assert_eq!(result["usage"]["completion_tokens"], 8);
    assert_eq!(result["usage"]["total_tokens"], 23);
}

#[test]
fn test_roundtrip_openai_to_claude_tool_call() {
    let reg = build_registry();

    // 1. Translate OpenAI request with tools → Claude format
    let openai_req = json!({
        "model": "gpt-4",
        "messages": [{"role": "user", "content": "What's the weather in SF?"}],
        "tools": [{
            "type": "function",
            "function": {
                "name": "get_weather",
                "description": "Get weather info",
                "parameters": {
                    "type": "object",
                    "properties": {"city": {"type": "string"}},
                    "required": ["city"]
                }
            }
        }]
    });
    let raw = serde_json::to_vec(&openai_req).unwrap();
    let claude_req = reg
        .translate_request(
            Format::OpenAI,
            Format::Claude,
            "claude-3-5-sonnet-20241022",
            &raw,
            false,
        )
        .unwrap();
    let claude_req_val: Value = serde_json::from_slice(&claude_req).unwrap();

    // Verify tools translated correctly
    assert_eq!(claude_req_val["tools"][0]["name"], "get_weather");
    assert!(claude_req_val["tools"][0]["input_schema"].is_object());

    // 2. Simulate Claude tool_use response
    let claude_resp = json!({
        "id": "msg_tool_rt",
        "model": "claude-3-5-sonnet-20241022",
        "content": [{
            "type": "tool_use",
            "id": "toolu_rt_1",
            "name": "get_weather",
            "input": {"city": "SF"}
        }],
        "stop_reason": "tool_use",
        "usage": {"input_tokens": 30, "output_tokens": 20}
    });
    let resp_data = serde_json::to_vec(&claude_resp).unwrap();

    // 3. Translate back to OpenAI
    let openai_resp = reg
        .translate_non_stream(
            Format::OpenAI,
            Format::Claude,
            "claude-3-5-sonnet-20241022",
            &raw,
            &resp_data,
        )
        .unwrap();
    let result: Value = serde_json::from_str(&openai_resp).unwrap();

    // 4. Verify
    assert_eq!(result["choices"][0]["finish_reason"], "tool_calls");
    assert_eq!(result["choices"][0]["message"]["content"], Value::Null);
    let tc = &result["choices"][0]["message"]["tool_calls"][0];
    assert_eq!(tc["id"], "toolu_rt_1");
    assert_eq!(tc["function"]["name"], "get_weather");
    let args: Value = serde_json::from_str(tc["function"]["arguments"].as_str().unwrap()).unwrap();
    assert_json_eq!(args, json!({"city": "SF"}));
}

#[test]
fn test_roundtrip_openai_to_gemini_text() {
    let reg = build_registry();

    // 1. Translate OpenAI request → Gemini format
    let openai_req = json!({
        "model": "gpt-4",
        "messages": [
            {"role": "system", "content": "Be concise."},
            {"role": "user", "content": "What is Rust?"}
        ],
        "temperature": 0.5
    });
    let raw = serde_json::to_vec(&openai_req).unwrap();
    let gemini_req = reg
        .translate_request(
            Format::OpenAI,
            Format::Gemini,
            "gemini-1.5-pro",
            &raw,
            false,
        )
        .unwrap();
    let gemini_req_val: Value = serde_json::from_slice(&gemini_req).unwrap();

    // Verify Gemini request
    assert!(gemini_req_val["systemInstruction"].is_object());
    assert_eq!(gemini_req_val["contents"][0]["role"], "user");
    assert_eq!(gemini_req_val["generationConfig"]["temperature"], 0.5);

    // 2. Simulate Gemini response
    let gemini_resp = json!({
        "candidates": [{
            "content": {
                "parts": [{"text": "Rust is a systems programming language."}],
                "role": "model"
            },
            "finishReason": "STOP"
        }],
        "modelVersion": "gemini-1.5-pro",
        "usageMetadata": {
            "promptTokenCount": 12,
            "candidatesTokenCount": 7,
            "totalTokenCount": 19
        }
    });
    let resp_data = serde_json::to_vec(&gemini_resp).unwrap();

    // 3. Translate back to OpenAI
    let openai_resp = reg
        .translate_non_stream(
            Format::OpenAI,
            Format::Gemini,
            "gemini-1.5-pro",
            &raw,
            &resp_data,
        )
        .unwrap();
    let result: Value = serde_json::from_str(&openai_resp).unwrap();

    // 4. Verify
    assert_eq!(result["object"], "chat.completion");
    assert_eq!(result["model"], "gemini-1.5-pro");
    assert_eq!(
        result["choices"][0]["message"]["content"],
        "Rust is a systems programming language."
    );
    assert_eq!(result["choices"][0]["finish_reason"], "stop");
    assert_eq!(result["usage"]["prompt_tokens"], 12);
    assert_eq!(result["usage"]["completion_tokens"], 7);
}

#[test]
fn test_roundtrip_openai_to_claude_streaming() {
    let reg = build_registry();

    // 1. Translate request
    let openai_req = json!({
        "model": "gpt-4",
        "messages": [{"role": "user", "content": "Say hi"}]
    });
    let raw = serde_json::to_vec(&openai_req).unwrap();
    let _claude_req = reg
        .translate_request(
            Format::OpenAI,
            Format::Claude,
            "claude-3-5-sonnet-20241022",
            &raw,
            true,
        )
        .unwrap();

    // 2. Simulate streaming Claude events
    let mut state = TranslateState::default();

    // message_start
    let msg_start = json!({
        "type": "message_start",
        "message": {
            "id": "msg_stream_rt",
            "model": "claude-3-5-sonnet-20241022",
            "usage": {"input_tokens": 5}
        }
    });
    let chunks = reg
        .translate_stream(
            Format::OpenAI,
            Format::Claude,
            "claude-3-5-sonnet-20241022",
            &raw,
            Some("message_start"),
            &serde_json::to_vec(&msg_start).unwrap(),
            &mut state,
        )
        .unwrap();
    assert_eq!(chunks.len(), 1);
    let c: Value = serde_json::from_str(&chunks[0]).unwrap();
    assert_eq!(c["choices"][0]["delta"]["role"], "assistant");

    // content_block_delta with text
    let text_delta = json!({
        "type": "content_block_delta",
        "delta": {"type": "text_delta", "text": "Hello!"}
    });
    let chunks = reg
        .translate_stream(
            Format::OpenAI,
            Format::Claude,
            "claude-3-5-sonnet-20241022",
            &raw,
            Some("content_block_delta"),
            &serde_json::to_vec(&text_delta).unwrap(),
            &mut state,
        )
        .unwrap();
    assert_eq!(chunks.len(), 1);
    let c: Value = serde_json::from_str(&chunks[0]).unwrap();
    assert_eq!(c["choices"][0]["delta"]["content"], "Hello!");

    // message_stop
    let msg_stop = json!({"type": "message_stop"});
    let chunks = reg
        .translate_stream(
            Format::OpenAI,
            Format::Claude,
            "claude-3-5-sonnet-20241022",
            &raw,
            Some("message_stop"),
            &serde_json::to_vec(&msg_stop).unwrap(),
            &mut state,
        )
        .unwrap();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0], "[DONE]");
}

#[test]
fn test_roundtrip_openai_compat_passthrough() {
    let reg = build_registry();

    // Request should just replace model
    let openai_req = json!({
        "model": "gpt-4",
        "messages": [{"role": "user", "content": "Hi"}],
        "temperature": 0.8,
        "stream": true
    });
    let raw = serde_json::to_vec(&openai_req).unwrap();
    let result = reg
        .translate_request(
            Format::OpenAI,
            Format::OpenAICompat,
            "deepseek-chat",
            &raw,
            true,
        )
        .unwrap();
    let val: Value = serde_json::from_slice(&result).unwrap();
    assert_eq!(val["model"], "deepseek-chat");
    assert_eq!(val["temperature"], 0.8);
    assert_eq!(val["messages"][0]["content"], "Hi");

    // Response passthrough
    let resp_data = b"{\"choices\":[{\"message\":{\"content\":\"Hello\"}}]}";
    let result = reg
        .translate_non_stream(
            Format::OpenAI,
            Format::OpenAICompat,
            "deepseek-chat",
            &raw,
            resp_data,
        )
        .unwrap();
    assert_eq!(result, String::from_utf8_lossy(resp_data));

    // Stream passthrough
    let mut state = TranslateState::default();
    let stream_data = b"{\"choices\":[{\"delta\":{\"content\":\"Hi\"}}]}";
    let chunks = reg
        .translate_stream(
            Format::OpenAI,
            Format::OpenAICompat,
            "deepseek-chat",
            &raw,
            None,
            stream_data,
            &mut state,
        )
        .unwrap();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0], String::from_utf8_lossy(stream_data));
}
