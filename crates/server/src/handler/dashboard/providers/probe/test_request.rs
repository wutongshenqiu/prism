use std::time::Instant;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::AppState;

use super::codex::{build_codex_probe_request, collect_codex_probe_response};
use super::common::{
    apply_auth_headers, build_reqwest_client, client_error_response, normalize_base_url,
    provider_name_from_config, select_runtime_auth,
};

#[derive(Debug, Deserialize)]
pub struct ProviderTestRequest {
    pub model: String,
    pub input: String,
}

fn parse_body(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|_| json!({ "raw": raw }))
}

pub async fn test_request(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(body): Json<ProviderTestRequest>,
) -> impl IntoResponse {
    if body.model.trim().is_empty() || body.input.trim().is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "error": "validation_failed",
                "message": "model and input are required",
            })),
        )
            .into_response();
    }

    let provider_name = match provider_name_from_config(&state, &name) {
        Ok(name) => name,
        Err(response) => return response.into_response(),
    };

    let Some(auth) = select_runtime_auth(&state, &provider_name) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "missing_auth",
                "message": "No runtime credential found for provider",
            })),
        )
            .into_response();
    };

    if let Err(err) = state.auth_runtime.prepare_auth(&state, &auth).await {
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({"status": "error", "message": err.to_string()})),
        )
            .into_response();
    }

    if auth.current_secret().trim().is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"status": "error", "message": "Provider credential is disconnected"})),
        )
            .into_response();
    }

    let default_proxy = state.config.load().proxy_url.clone();
    let proxy_url = auth.effective_proxy(default_proxy.as_deref());
    let client = match build_reqwest_client(&state.http_client_pool, proxy_url, 45) {
        Ok(client) => client,
        Err(message) => return client_error_response(message).into_response(),
    };

    let base = normalize_base_url(&auth.resolved_base_url()).to_string();
    let model = body.model.trim().to_string();
    let input = body.input.trim().to_string();

    let (endpoint, request_body, request_builder, is_codex_stream) =
        match (auth.provider, auth.upstream) {
            (prism_core::provider::Format::OpenAI, prism_core::provider::UpstreamKind::Codex) => {
                let payload = json!({
                    "model": model,
                    "instructions": "",
                    "input": [{
                        "role": "user",
                        "content": [{
                            "type": "input_text",
                            "text": input,
                        }]
                    }],
                    "store": false,
                    "stream": true,
                });
                let endpoint = format!("{}/responses", auth.resolved_base_url());
                let request = build_codex_probe_request(&client, &auth, &payload);
                (endpoint, payload, request, true)
            }
            (prism_core::provider::Format::OpenAI, _) => match auth.wire_api {
                prism_core::provider::WireApi::Responses => {
                    let payload = json!({
                        "model": model,
                        "input": input,
                        "store": false,
                    });
                    let endpoint = format!("{base}/v1/responses");
                    let request = apply_auth_headers(client.post(&endpoint).json(&payload), &auth);
                    (endpoint, payload, request, false)
                }
                prism_core::provider::WireApi::Chat => {
                    let payload = json!({
                        "model": model,
                        "stream": false,
                        "max_tokens": 256,
                        "messages": [{ "role": "user", "content": input }],
                    });
                    let endpoint = format!("{base}/v1/chat/completions");
                    let request = apply_auth_headers(client.post(&endpoint).json(&payload), &auth);
                    (endpoint, payload, request, false)
                }
            },
            (prism_core::provider::Format::Claude, _) => {
                let payload = json!({
                    "model": model,
                    "max_tokens": 256,
                    "messages": [{ "role": "user", "content": input }],
                });
                let endpoint = format!("{base}/v1/messages");
                let request = apply_auth_headers(
                    client
                        .post(&endpoint)
                        .header("anthropic-version", "2023-06-01")
                        .json(&payload),
                    &auth,
                );
                (endpoint, payload, request, false)
            }
            (prism_core::provider::Format::Gemini, _) => {
                let payload = json!({
                    "contents": [{
                        "role": "user",
                        "parts": [{ "text": input }],
                    }],
                    "generationConfig": {
                        "maxOutputTokens": 256,
                    }
                });
                let endpoint = format!("{base}/v1beta/models/{model}:generateContent");
                let request = apply_auth_headers(client.post(&endpoint).json(&payload), &auth);
                (endpoint, payload, request, false)
            }
        };

    let started = Instant::now();
    match request_builder.send().await {
        Ok(response) => {
            let status = response.status();
            let response_body = if is_codex_stream {
                match collect_codex_probe_response(response).await {
                    Ok((_saw_delta, payload)) => payload,
                    Err(error) => {
                        return (
                            StatusCode::BAD_GATEWAY,
                            Json(json!({
                                "error": "upstream_request_failed",
                                "message": error,
                            })),
                        )
                            .into_response();
                    }
                }
            } else {
                let raw_body = match response.text().await {
                    Ok(text) => text,
                    Err(error) => {
                        return (
                            StatusCode::BAD_GATEWAY,
                            Json(json!({
                                "error": "upstream_read_failed",
                                "message": error.to_string(),
                            })),
                        )
                            .into_response();
                    }
                };
                parse_body(&raw_body)
            };

            (
                StatusCode::OK,
                Json(json!({
                    "provider": provider_name,
                    "upstream": auth.upstream.to_string(),
                    "endpoint": endpoint,
                    "format": auth.provider.as_str(),
                    "model": model,
                    "status": status.as_u16(),
                    "ok": status.is_success(),
                    "latency_ms": started.elapsed().as_millis() as u64,
                    "request_body": request_body,
                    "response_body": response_body,
                })),
            )
                .into_response()
        }
        Err(error) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({
                "error": "upstream_request_failed",
                "message": error.to_string(),
            })),
        )
            .into_response(),
    }
}
