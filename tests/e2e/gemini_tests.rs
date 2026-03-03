use crate::helpers::*;

/// Gemini via OpenAI format (cross-format translation).
#[tokio::test]
#[ignore]
async fn gemini_via_openai_non_streaming() {
    let api_key = require_env!("E2E_GEMINI_API_KEY");
    let config = build_gemini_config(&api_key);
    let server = TestServer::start(config).await;
    let client = http_client();

    let resp = client
        .post(format!("{}/v1/chat/completions", server.base_url))
        .json(&serde_json::json!({
            "model": "gemini-2.0-flash",
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

    // Should return OpenAI format via translation
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
