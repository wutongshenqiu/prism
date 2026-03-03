use crate::helpers::*;

/// Claude native Messages API non-streaming.
#[tokio::test]
#[ignore]
async fn claude_messages_non_streaming() {
    let api_key = require_env!("E2E_CLAUDE_API_KEY");
    let config = build_claude_config(&api_key);
    let server = TestServer::start(config).await;
    let client = http_client();

    let resp = client
        .post(format!("{}/v1/messages", server.base_url))
        .header("x-api-key", "dummy")
        .header("anthropic-version", "2023-06-01")
        .json(&serde_json::json!({
            "model": "claude-sonnet-4-20250514",
            "max_tokens": 20,
            "messages": [{"role": "user", "content": "Say hello in one word."}],
        }))
        .send()
        .await
        .expect("request failed");

    assert!(
        resp.status().is_success(),
        "status: {}, body: {}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );

    let body: serde_json::Value =
        serde_json::from_str(&resp.text().await.unwrap()).expect("invalid JSON");

    assert_eq!(body["type"], "message");
    assert_eq!(body["role"], "assistant");
    assert!(body["content"].is_array(), "content should be array");
    let block = &body["content"][0];
    assert_eq!(block["type"], "text");
    assert!(
        block["text"].as_str().is_some_and(|s| !s.is_empty()),
        "text should be non-empty"
    );
    assert!(body["usage"]["input_tokens"].as_u64().unwrap_or(0) > 0);
    assert!(body["usage"]["output_tokens"].as_u64().unwrap_or(0) > 0);
}

/// Claude native Messages API streaming.
#[tokio::test]
#[ignore]
async fn claude_messages_streaming() {
    let api_key = require_env!("E2E_CLAUDE_API_KEY");
    let config = build_claude_config(&api_key);
    let server = TestServer::start(config).await;
    let client = http_client();

    let resp = client
        .post(format!("{}/v1/messages", server.base_url))
        .header("x-api-key", "dummy")
        .header("anthropic-version", "2023-06-01")
        .json(&serde_json::json!({
            "model": "claude-sonnet-4-20250514",
            "max_tokens": 20,
            "messages": [{"role": "user", "content": "Say hi."}],
            "stream": true,
        }))
        .send()
        .await
        .expect("request failed");

    assert!(resp.status().is_success(), "status: {}", resp.status());

    let body = resp.text().await.expect("failed to read body");
    let events = parse_sse_events(&body);

    assert!(!events.is_empty(), "should have SSE events");

    // Claude SSE: expect message_start, content_block_start, content_block_delta, etc.
    let mut found_delta = false;
    let mut found_stop = false;
    for (event_type, data) in &events {
        if event_type.as_deref() == Some("content_block_delta")
            && let Ok(v) = serde_json::from_str::<serde_json::Value>(data)
            && v["delta"]["type"] == "text_delta"
        {
            found_delta = true;
        }
        if event_type.as_deref() == Some("message_stop") {
            found_stop = true;
        }
    }
    assert!(found_delta, "should have content_block_delta events");
    assert!(found_stop, "should have message_stop event");
}

/// Claude via OpenAI format (cross-format translation).
/// Sends an OpenAI-format request, proxy translates to Claude, translates response back.
#[tokio::test]
#[ignore]
async fn claude_via_openai_format() {
    let api_key = require_env!("E2E_CLAUDE_API_KEY");
    let config = build_claude_config(&api_key);
    let server = TestServer::start(config).await;
    let client = http_client();

    let resp = client
        .post(format!("{}/v1/chat/completions", server.base_url))
        .json(&serde_json::json!({
            "model": "claude-sonnet-4-20250514",
            "messages": [{"role": "user", "content": "Say hello in one word."}],
            "max_tokens": 20,
        }))
        .send()
        .await
        .expect("request failed");

    assert!(
        resp.status().is_success(),
        "status: {}, body: {}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );

    let body: serde_json::Value =
        serde_json::from_str(&resp.text().await.unwrap()).expect("invalid JSON");

    // Should return OpenAI format despite using Claude backend
    assert_eq!(body["object"], "chat.completion");
    assert!(body["choices"].is_array());
    let choice = &body["choices"][0];
    assert_eq!(choice["message"]["role"], "assistant");
    assert!(
        choice["message"]["content"]
            .as_str()
            .is_some_and(|s| !s.is_empty()),
        "content should be non-empty"
    );
}
