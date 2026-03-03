use crate::helpers::*;

/// OpenAI non-streaming chat completion.
#[tokio::test]
#[ignore]
async fn openai_chat_non_streaming() {
    let api_key = require_env!("E2E_OPENAI_API_KEY");
    let config = build_openai_config(&api_key);
    let server = TestServer::start(config).await;
    let client = http_client();

    let resp = client
        .post(format!("{}/v1/chat/completions", server.base_url))
        .json(&serde_json::json!({
            "model": "gpt-4o-mini",
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
    assert!(body["usage"]["prompt_tokens"].as_u64().unwrap_or(0) > 0);
    assert!(body["usage"]["completion_tokens"].as_u64().unwrap_or(0) > 0);
}

/// OpenAI streaming chat completion.
#[tokio::test]
#[ignore]
async fn openai_chat_streaming() {
    let api_key = require_env!("E2E_OPENAI_API_KEY");
    let config = build_openai_config(&api_key);
    let server = TestServer::start(config).await;
    let client = http_client();

    let resp = client
        .post(format!("{}/v1/chat/completions", server.base_url))
        .json(&serde_json::json!({
            "model": "gpt-4o-mini",
            "messages": [{"role": "user", "content": "Say hi."}],
            "max_tokens": 10,
            "stream": true,
        }))
        .send()
        .await
        .expect("request failed");

    assert!(resp.status().is_success(), "status: {}", resp.status());

    let body = resp.text().await.expect("failed to read body");
    let events = parse_sse_events(&body);

    assert!(!events.is_empty(), "should have SSE events");

    let last_data = &events.last().unwrap().1;
    assert_eq!(last_data, "[DONE]", "last event should be [DONE]");

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
