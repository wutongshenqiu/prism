use crate::helpers::*;

/// Bailian (Alibaba Cloud) non-streaming chat completion via OpenAI-compat.
#[tokio::test]
#[ignore]
async fn bailian_chat_non_streaming() {
    let api_key = require_env!("E2E_BAILIAN_API_KEY");
    let model = bailian_model(&api_key);
    let config = build_bailian_config(&api_key);
    let server = TestServer::start(config).await;
    let client = http_client();

    let resp = client
        .post(format!("{}/v1/chat/completions", server.base_url))
        .json(&serde_json::json!({
            "model": model,
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

    // Verify response structure
    assert_eq!(body["object"], "chat.completion");
    assert!(body["choices"].is_array(), "choices should be array");
    let choice = &body["choices"][0];
    assert_eq!(choice["message"]["role"], "assistant");
    assert!(
        choice["message"]["content"]
            .as_str()
            .is_some_and(|s| !s.is_empty()),
        "content should be non-empty"
    );
    assert!(body["usage"]["prompt_tokens"].as_u64().unwrap_or(0) > 0);
    assert!(body["usage"]["completion_tokens"].as_u64().unwrap_or(0) > 0);
}

/// Bailian streaming chat completion via OpenAI-compat.
#[tokio::test]
#[ignore]
async fn bailian_chat_streaming() {
    let api_key = require_env!("E2E_BAILIAN_API_KEY");
    let model = bailian_model(&api_key);
    let config = build_bailian_config(&api_key);
    let server = TestServer::start(config).await;
    let client = http_client();

    let resp = client
        .post(format!("{}/v1/chat/completions", server.base_url))
        .json(&serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": "Say hi."}],
            "max_tokens": 10,
            "stream": true,
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

    let body = resp.text().await.expect("failed to read body");
    let events = parse_sse_events(&body);

    assert!(!events.is_empty(), "should have SSE events");

    // Last event should be [DONE]
    let last_data = &events.last().unwrap().1;
    assert_eq!(last_data, "[DONE]", "last event should be [DONE]");

    // At least one event (before [DONE]) should have delta content
    let mut found_content = false;
    for (_, data) in &events {
        if data == "[DONE]" {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(data)
            && v["choices"][0]["delta"]["content"].as_str().is_some()
        {
            found_content = true;
        }
    }
    assert!(
        found_content,
        "should have at least one chunk with delta.content"
    );
}

/// Bailian model listing via OpenAI-compat.
#[tokio::test]
#[ignore]
async fn bailian_model_listing() {
    let api_key = require_env!("E2E_BAILIAN_API_KEY");
    let config = build_bailian_config(&api_key);
    let server = TestServer::start(config).await;
    let client = http_client();

    let resp = client
        .get(format!("{}/v1/models", server.base_url))
        .send()
        .await
        .expect("request failed");

    assert!(resp.status().is_success(), "status: {}", resp.status());

    let body: serde_json::Value =
        serde_json::from_str(&resp.text().await.unwrap()).expect("invalid JSON");

    assert_eq!(body["object"], "list");
    assert!(body["data"].is_array(), "data should be array");
}
