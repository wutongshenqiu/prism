use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use prism_provider::sse::parse_sse_stream;
use serde::Deserialize;
use serde_json::json;
use std::time::Instant;
use tokio_stream::StreamExt;

use super::{
    ProbeStatus, ProviderProbeCheck, ProviderProbeResult, is_valid_format, parse_upstream_kind,
};

#[derive(Debug, Deserialize)]
pub struct FetchModelsRequest {
    pub format: String,
    #[serde(default)]
    pub upstream: Option<String>,
    pub api_key: String,
    #[serde(default)]
    pub base_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PresentationPreviewRequest {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub user_agent: Option<String>,
    #[serde(default)]
    pub sample_body: Option<serde_json::Value>,
}

const CODEX_HEALTH_USER_AGENT: &str =
    "codex_cli_rs/0.101.0 (Mac OS 26.0.1; arm64) Apple_Terminal/464";
const RED_DOT_PNG_DATA_URL: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC";

fn build_reqwest_client(
    pool: &prism_core::proxy::HttpClientPool,
    proxy_url: Option<&str>,
    timeout_secs: u64,
) -> Result<reqwest::Client, String> {
    pool.get_or_create(None, proxy_url, timeout_secs, timeout_secs)
        .map_err(|e| format!("Failed to build HTTP client: {e}"))
}

fn default_base_url(upstream: prism_core::provider::UpstreamKind) -> &'static str {
    upstream.default_base_url()
}

fn normalize_base_url(base_url: &str) -> &str {
    let url = base_url.trim_end_matches('/');
    if let Some(stripped) = url.strip_suffix("/v1") {
        stripped
    } else if let Some(stripped) = url.strip_suffix("/v1beta") {
        stripped
    } else {
        url
    }
}

fn build_models_request(
    client: &reqwest::Client,
    provider_type: &str,
    api_key: &str,
    base_url: &str,
    extra_headers: Option<&std::collections::HashMap<String, String>>,
) -> Result<reqwest::RequestBuilder, String> {
    let base = normalize_base_url(base_url);
    let mut req = match provider_type {
        "openai" => client
            .get(format!("{base}/v1/models"))
            .header("Authorization", format!("Bearer {api_key}")),
        "claude" => client
            .get(format!("{base}/v1/models"))
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01"),
        "gemini" => client
            .get(format!("{base}/v1beta/models"))
            .header("x-goog-api-key", api_key),
        _ => return Err(format!("Unsupported provider_type: {provider_type}")),
    };
    if let Some(headers) = extra_headers {
        for (k, v) in headers {
            req = req.header(k.as_str(), v.as_str());
        }
    }
    Ok(req)
}

fn extract_model_ids(provider_type: &str, body: &serde_json::Value) -> Vec<String> {
    match provider_type {
        "openai" | "claude" => body
            .get("data")
            .and_then(|d| d.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| item.get("id").and_then(|v| v.as_str()).map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        "gemini" => body
            .get("models")
            .and_then(|m| m.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        item.get("name")
                            .and_then(|v| v.as_str())
                            .map(|s| s.strip_prefix("models/").unwrap_or(s).to_string())
                    })
                    .collect()
            })
            .unwrap_or_default(),
        _ => vec![],
    }
}

fn select_health_auth(
    state: &AppState,
    provider_name: &str,
) -> Option<prism_core::provider::AuthRecord> {
    state
        .router
        .credential_map()
        .get(provider_name)
        .and_then(|records| {
            records
                .iter()
                .find(|record| !record.disabled)
                .cloned()
                .or_else(|| records.first().cloned())
        })
}

pub fn cached_probe_result(state: &AppState, provider_name: &str) -> Option<ProviderProbeResult> {
    state
        .provider_probe_cache
        .get(provider_name)
        .map(|entry| entry.value().clone())
}

fn build_codex_probe_request(
    client: &reqwest::Client,
    auth: &prism_core::provider::AuthRecord,
    body: &serde_json::Value,
) -> reqwest::RequestBuilder {
    let body = serde_json::to_vec(body).unwrap_or_else(|_| b"{}".to_vec());
    let mut req = client
        .post(format!("{}/responses", auth.resolved_base_url()))
        .header("content-type", "application/json")
        .header("accept", "text/event-stream")
        .header("authorization", format!("Bearer {}", auth.current_secret()))
        .header("version", "0.101.0")
        .header("session_id", uuid::Uuid::new_v4().to_string())
        .header("originator", "codex_cli_rs")
        .body(body);
    if let Some(account_id) = auth.current_account_id()
        && !account_id.trim().is_empty()
    {
        req = req.header("chatgpt-account-id", account_id);
    }
    if !auth
        .headers
        .keys()
        .any(|key| key.eq_ignore_ascii_case("user-agent"))
    {
        req = req.header("user-agent", CODEX_HEALTH_USER_AGENT);
    }
    for (k, v) in &auth.headers {
        if prism_core::presentation::protected::is_protected(k) {
            continue;
        }
        req = req.header(k.as_str(), v.as_str());
    }
    req
}

fn codex_probe_model(auth: &prism_core::provider::AuthRecord) -> String {
    auth.models
        .first()
        .map(|entry| entry.id.clone())
        .unwrap_or_else(|| "gpt-5".to_string())
}

fn probe_check(
    capability: &str,
    status: ProbeStatus,
    message: impl Into<Option<String>>,
) -> ProviderProbeCheck {
    ProviderProbeCheck {
        capability: capability.to_string(),
        status,
        message: message.into(),
    }
}

async fn collect_codex_probe_response(
    resp: reqwest::Response,
) -> Result<(bool, serde_json::Value), String> {
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("upstream returned {status}: {body}"));
    }

    let mut saw_delta = false;
    let mut sse_stream = parse_sse_stream(resp.bytes_stream());
    while let Some(event) = sse_stream.next().await {
        let event = event.map_err(|e| e.to_string())?;
        if event.data == "[DONE]" {
            continue;
        }
        let payload: serde_json::Value =
            serde_json::from_str(&event.data).map_err(|e| format!("invalid SSE payload: {e}"))?;
        let event_type = payload
            .get("type")
            .and_then(|value| value.as_str())
            .or(event.event.as_deref())
            .unwrap_or("");
        if event_type == "response.output_text.delta" {
            saw_delta = true;
        }
        if event_type == "response.completed" {
            let response = payload
                .get("response")
                .cloned()
                .ok_or_else(|| "response.completed missing response payload".to_string())?;
            return Ok((saw_delta, response));
        }
    }

    Err("stream ended before response.completed".to_string())
}

fn extract_response_text(payload: &serde_json::Value) -> String {
    payload
        .get("output")
        .and_then(|value| value.as_array())
        .map(|items| {
            let mut text = String::new();
            for item in items {
                if item.get("type").and_then(|value| value.as_str()) == Some("message")
                    && let Some(content) = item.get("content").and_then(|value| value.as_array())
                {
                    for part in content {
                        if part.get("type").and_then(|value| value.as_str()) == Some("output_text")
                            && let Some(value) = part.get("text").and_then(|value| value.as_str())
                        {
                            text.push_str(value);
                        }
                    }
                }
            }
            text
        })
        .unwrap_or_default()
}

async fn run_codex_probe(
    client: &reqwest::Client,
    auth: &prism_core::provider::AuthRecord,
) -> ProviderProbeResult {
    let started = Instant::now();
    let model = codex_probe_model(auth);

    let text_payload = json!({
        "model": model,
        "instructions": "",
        "input": [{
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": "Reply with exactly TEXT_PROBE_OK"
            }]
        }],
        "store": false,
        "stream": true,
    });
    let text_check = match build_codex_probe_request(client, auth, &text_payload)
        .send()
        .await
    {
        Ok(resp) => match collect_codex_probe_response(resp).await {
            Ok((_saw_delta, payload)) => {
                let text = extract_response_text(&payload);
                if text.contains("TEXT_PROBE_OK") {
                    probe_check("text", ProbeStatus::Verified, None)
                } else {
                    probe_check(
                        "text",
                        ProbeStatus::Failed,
                        Some(format!("unexpected text response: {text}")),
                    )
                }
            }
            Err(err) => probe_check("text", ProbeStatus::Failed, Some(err)),
        },
        Err(err) => probe_check("text", ProbeStatus::Failed, Some(err.to_string())),
    };

    let stream_payload = json!({
        "model": model,
        "instructions": "",
        "input": [{
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": "Reply with exactly STREAM_PROBE_OK"
            }]
        }],
        "store": false,
        "stream": true,
    });
    let stream_check = match build_codex_probe_request(client, auth, &stream_payload)
        .send()
        .await
    {
        Ok(resp) => match collect_codex_probe_response(resp).await {
            Ok((saw_delta, payload)) => {
                let text = extract_response_text(&payload);
                if saw_delta && text.contains("STREAM_PROBE_OK") {
                    probe_check("stream", ProbeStatus::Verified, None)
                } else if !saw_delta {
                    probe_check(
                        "stream",
                        ProbeStatus::Failed,
                        Some("no output_text delta event observed".to_string()),
                    )
                } else {
                    probe_check(
                        "stream",
                        ProbeStatus::Failed,
                        Some(format!("unexpected stream response: {text}")),
                    )
                }
            }
            Err(err) => probe_check("stream", ProbeStatus::Failed, Some(err)),
        },
        Err(err) => probe_check("stream", ProbeStatus::Failed, Some(err.to_string())),
    };

    let tools_payload = json!({
        "model": model,
        "instructions": "",
        "input": [{
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": "Call the probe_tool function exactly once."
            }]
        }],
        "tools": [{
            "type": "function",
            "name": "probe_tool",
            "description": "Health probe tool",
            "parameters": {
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }
        }],
        "tool_choice": "required",
        "store": false,
        "stream": true,
    });
    let tools_check = match build_codex_probe_request(client, auth, &tools_payload)
        .send()
        .await
    {
        Ok(resp) => match collect_codex_probe_response(resp).await {
            Ok((_saw_delta, payload)) => {
                let found = payload
                    .get("output")
                    .and_then(|value| value.as_array())
                    .map(|items| {
                        items.iter().any(|item| {
                            item.get("type")
                                .and_then(|value| value.as_str())
                                .is_some_and(|value| {
                                    value.contains("function_call") || value.contains("tool_call")
                                })
                                || item
                                    .get("name")
                                    .and_then(|value| value.as_str())
                                    .is_some_and(|value| value == "probe_tool")
                        })
                    })
                    .unwrap_or(false);
                if found {
                    probe_check("tools", ProbeStatus::Verified, None)
                } else {
                    probe_check(
                        "tools",
                        ProbeStatus::Failed,
                        Some("response completed without a tool call".to_string()),
                    )
                }
            }
            Err(err) => {
                let status = if err.contains("Unsupported") || err.contains("unknown_parameter") {
                    ProbeStatus::Unsupported
                } else {
                    ProbeStatus::Failed
                };
                probe_check("tools", status, Some(err))
            }
        },
        Err(err) => probe_check("tools", ProbeStatus::Failed, Some(err.to_string())),
    };

    let images_payload = json!({
        "model": model,
        "instructions": "",
        "input": [{
            "role": "user",
            "content": [
                {
                    "type": "input_text",
                    "text": "The image is a solid red square. Answer with exactly red."
                },
                {
                    "type": "input_image",
                    "image_url": RED_DOT_PNG_DATA_URL
                }
            ]
        }],
        "store": false,
        "stream": true,
    });
    let images_check = match build_codex_probe_request(client, auth, &images_payload)
        .send()
        .await
    {
        Ok(resp) => match collect_codex_probe_response(resp).await {
            Ok((_saw_delta, payload)) => {
                let text = extract_response_text(&payload).to_lowercase();
                if text.contains("red") {
                    probe_check("images", ProbeStatus::Verified, None)
                } else {
                    probe_check(
                        "images",
                        ProbeStatus::Failed,
                        Some(format!("unexpected image response: {text}")),
                    )
                }
            }
            Err(err) => {
                let status = if err.contains("Unsupported") || err.contains("unknown_parameter") {
                    ProbeStatus::Unsupported
                } else {
                    ProbeStatus::Failed
                };
                probe_check("images", status, Some(err))
            }
        },
        Err(err) => probe_check("images", ProbeStatus::Failed, Some(err.to_string())),
    };

    let checks = vec![
        text_check,
        stream_check,
        tools_check,
        images_check,
        probe_check(
            "json_schema",
            ProbeStatus::Unknown,
            Some("no live probe implemented".to_string()),
        ),
        probe_check(
            "reasoning",
            ProbeStatus::Unknown,
            Some("no live probe implemented".to_string()),
        ),
        probe_check(
            "count_tokens",
            ProbeStatus::Unsupported,
            Some("Codex backend does not expose count_tokens".to_string()),
        ),
    ];

    let status = if checks.iter().any(|check| {
        (check.capability == "text" || check.capability == "stream") && !check.status.is_verified()
    }) {
        "error"
    } else if checks.iter().all(|check| {
        matches!(
            check.status,
            ProbeStatus::Verified | ProbeStatus::Unsupported | ProbeStatus::Unknown
        )
    }) && checks.iter().any(|check| {
        matches!(
            check.status,
            ProbeStatus::Unsupported | ProbeStatus::Unknown
        )
    }) {
        "warning"
    } else {
        "ok"
    };

    ProviderProbeResult {
        provider: auth.provider_name.clone(),
        upstream: auth.upstream.to_string(),
        status: status.to_string(),
        checked_at: chrono::Utc::now().to_rfc3339(),
        latency_ms: started.elapsed().as_millis() as u64,
        checks,
    }
}

async fn run_generic_health_probe(
    provider_name: &str,
    auth: &prism_core::provider::AuthRecord,
    client: &reqwest::Client,
) -> ProviderProbeResult {
    let started = Instant::now();
    let response = build_models_request(
        client,
        auth.provider.as_str(),
        &auth.current_secret(),
        &auth.resolved_base_url(),
        Some(&auth.headers),
    );
    let auth_check = match response {
        Ok(req) => match req.send().await {
            Ok(resp) if resp.status().is_success() => {
                probe_check("auth", ProbeStatus::Verified, None)
            }
            Ok(resp) if matches!(resp.status().as_u16(), 401 | 403) => probe_check(
                "auth",
                ProbeStatus::Failed,
                Some("credential rejected by upstream".to_string()),
            ),
            Ok(resp) => probe_check(
                "auth",
                ProbeStatus::Failed,
                Some(format!("upstream returned {}", resp.status())),
            ),
            Err(err) => probe_check("auth", ProbeStatus::Failed, Some(err.to_string())),
        },
        Err(err) => probe_check("auth", ProbeStatus::Failed, Some(err)),
    };

    ProviderProbeResult {
        provider: provider_name.to_string(),
        upstream: auth.upstream.to_string(),
        status: if auth_check.status.is_verified() {
            "ok".to_string()
        } else {
            "error".to_string()
        },
        checked_at: chrono::Utc::now().to_rfc3339(),
        latency_ms: started.elapsed().as_millis() as u64,
        checks: vec![
            auth_check,
            probe_check(
                "text",
                ProbeStatus::Unknown,
                Some("no live probe implemented for this upstream".to_string()),
            ),
            probe_check(
                "stream",
                ProbeStatus::Unknown,
                Some("no live probe implemented for this upstream".to_string()),
            ),
            probe_check(
                "tools",
                ProbeStatus::Unknown,
                Some("no live probe implemented for this upstream".to_string()),
            ),
            probe_check(
                "images",
                ProbeStatus::Unknown,
                Some("no live probe implemented for this upstream".to_string()),
            ),
            probe_check(
                "json_schema",
                ProbeStatus::Unknown,
                Some("no live probe implemented for this upstream".to_string()),
            ),
            probe_check(
                "reasoning",
                ProbeStatus::Unknown,
                Some("no live probe implemented for this upstream".to_string()),
            ),
            probe_check(
                "count_tokens",
                ProbeStatus::Unknown,
                Some("no live probe implemented for this upstream".to_string()),
            ),
        ],
    }
}

pub async fn fetch_models(
    State(state): State<AppState>,
    Json(body): Json<FetchModelsRequest>,
) -> impl IntoResponse {
    let format = body.format.as_str();

    if !is_valid_format(format) {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(
                json!({"error": "validation_failed", "message": "Invalid format. Must be one of: openai, claude, gemini"}),
            ),
        );
    }
    let parsed_format: prism_core::provider::Format = match format.parse() {
        Ok(value) => value,
        Err(_) => prism_core::provider::Format::OpenAI,
    };
    let upstream = match parse_upstream_kind(parsed_format, body.upstream.as_deref()) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if upstream == prism_core::provider::UpstreamKind::Codex {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "error": "validation_failed",
                "message": "Codex upstream does not support model discovery; configure models manually"
            })),
        );
    }

    let base_url = match body.base_url.as_deref().filter(|s| !s.is_empty()) {
        Some(url) => url.to_string(),
        None => default_base_url(upstream).to_string(),
    };

    let global_proxy = state.config.load().proxy_url.clone();
    let client = match build_reqwest_client(&state.http_client_pool, global_proxy.as_deref(), 15) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "client_error", "message": e})),
            );
        }
    };

    let request = match build_models_request(&client, format, &body.api_key, &base_url, None) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({"error": "validation_failed", "message": e})),
            );
        }
    };

    let response: reqwest::Response = match request.send().await {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(
                    json!({"error": "upstream_error", "message": format!("Failed to reach upstream: {e}")}),
                ),
            );
        }
    };

    if !response.status().is_success() {
        let status = response.status();
        let body_text = response.text().await.unwrap_or_default();
        return (
            StatusCode::BAD_GATEWAY,
            Json(
                json!({"error": "upstream_error", "message": format!("Upstream returned {status}: {body_text}")}),
            ),
        );
    }

    let body_json: serde_json::Value = match response.json().await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(
                    json!({"error": "upstream_error", "message": format!("Failed to parse upstream response: {e}")}),
                ),
            );
        }
    };

    let models = extract_model_ids(format, &body_json);
    (StatusCode::OK, Json(json!({"models": models})))
}

pub async fn presentation_preview(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(body): Json<PresentationPreviewRequest>,
) -> impl IntoResponse {
    let config = state.config.load();

    let entry = match config.providers.iter().find(|e| e.name == name) {
        Some(e) => e,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "not_found", "message": "Provider not found"})),
            );
        }
    };

    let mut payload = body
        .sample_body
        .unwrap_or_else(|| json!({"messages": [{"role": "user", "content": "hello"}]}));

    let ctx = prism_core::presentation::PresentationContext {
        target_format: entry.format,
        model: body.model.as_deref().unwrap_or("unknown"),
        user_agent: body.user_agent.as_deref(),
        api_key: &entry.api_key,
    };

    let result = prism_core::presentation::apply(&entry.upstream_presentation, &ctx, &mut payload);

    (
        StatusCode::OK,
        Json(json!({
            "profile": result.trace.profile,
            "activated": result.trace.activated,
            "effective_headers": result.headers,
            "body_mutations": result.trace.body_mutations,
            "protected_headers_blocked": result.trace.protected_blocked,
            "effective_body": payload,
        })),
    )
}

pub async fn health_check(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let provider_name = {
        let config = state.config.load();
        match config.providers.iter().find(|entry| entry.name == name) {
            Some(entry) => entry.name.clone(),
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({"status": "error", "message": "Provider not found"})),
                );
            }
        }
    };

    let Some(auth) = select_health_auth(&state, &provider_name) else {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(
                json!({"status": "error", "message": "No credential configured for this provider"}),
            ),
        );
    };
    if let Err(err) = state.auth_runtime.prepare_auth(&state, &auth).await {
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({"status": "error", "message": err.to_string()})),
        );
    }
    if auth.current_secret().trim().is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"status": "error", "message": "Provider credential is disconnected"})),
        );
    }

    let default_proxy = state.config.load().proxy_url.clone();
    let proxy_url = auth.effective_proxy(default_proxy.as_deref());

    let client = match build_reqwest_client(&state.http_client_pool, proxy_url, 10) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"status": "error", "message": e})),
            );
        }
    };

    let result = if auth.upstream == prism_core::provider::UpstreamKind::Codex {
        run_codex_probe(&client, &auth).await
    } else {
        run_generic_health_probe(&provider_name, &auth, &client).await
    };
    state
        .provider_probe_cache
        .insert(provider_name, result.clone());

    (StatusCode::OK, Json(json!(result)))
}
