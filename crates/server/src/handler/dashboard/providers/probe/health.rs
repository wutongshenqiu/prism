use std::time::Instant;

use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde_json::json;

use super::super::{ProbeStatus, ProviderProbeResult};
use super::codex::run_codex_probe;
use super::common::{
    apply_auth_headers, build_reqwest_client, client_error_response, normalize_base_url,
    probe_check, provider_name_from_config, select_runtime_auth,
};

fn configured_probe_model(auth: &prism_core::provider::AuthRecord) -> Option<&str> {
    auth.models
        .iter()
        .find(|entry| !entry.id.trim().is_empty())
        .map(|entry| entry.id.as_str())
}

fn summarize_generic_status(
    auth_check: &crate::handler::dashboard::providers::ProviderProbeCheck,
    text_check: &crate::handler::dashboard::providers::ProviderProbeCheck,
) -> &'static str {
    if matches!(auth_check.status, ProbeStatus::Failed)
        || matches!(text_check.status, ProbeStatus::Failed)
    {
        "error"
    } else if auth_check.status.is_verified() && text_check.status.is_verified() {
        "ok"
    } else {
        "warning"
    }
}

async fn run_openai_text_probe(
    client: &reqwest::Client,
    auth: &prism_core::provider::AuthRecord,
    model: &str,
) -> (
    crate::handler::dashboard::providers::ProviderProbeCheck,
    crate::handler::dashboard::providers::ProviderProbeCheck,
) {
    let resolved_base_url = auth.resolved_base_url();
    let base = normalize_base_url(&resolved_base_url);
    let payload = match auth.wire_api {
        prism_core::provider::WireApi::Responses => json!({
            "model": model,
            "input": "Reply with exactly ok.",
            "store": false,
        }),
        prism_core::provider::WireApi::Chat => json!({
            "model": model,
            "stream": false,
            "max_tokens": 4,
            "messages": [{ "role": "user", "content": "Reply with exactly ok." }],
        }),
    };
    let endpoint = match auth.wire_api {
        prism_core::provider::WireApi::Responses => format!("{base}/v1/responses"),
        prism_core::provider::WireApi::Chat => format!("{base}/v1/chat/completions"),
    };
    let request = apply_auth_headers(client.post(endpoint).json(&payload), auth);
    match request.send().await {
        Ok(response) if response.status().is_success() => (
            probe_check("auth", ProbeStatus::Verified, None),
            probe_check("text", ProbeStatus::Verified, None),
        ),
        Ok(response) if matches!(response.status().as_u16(), 401 | 403) => (
            probe_check(
                "auth",
                ProbeStatus::Failed,
                Some("credential rejected by upstream".to_string()),
            ),
            probe_check(
                "text",
                ProbeStatus::Unknown,
                Some("text probe aborted after authentication failure".to_string()),
            ),
        ),
        Ok(response) => (
            probe_check("auth", ProbeStatus::Verified, None),
            probe_check(
                "text",
                ProbeStatus::Failed,
                Some(format!("upstream returned {}", response.status())),
            ),
        ),
        Err(error) => (
            probe_check(
                "auth",
                ProbeStatus::Unknown,
                Some("text probe failed before upstream confirmed auth".to_string()),
            ),
            probe_check("text", ProbeStatus::Failed, Some(error.to_string())),
        ),
    }
}

async fn run_claude_text_probe(
    client: &reqwest::Client,
    auth: &prism_core::provider::AuthRecord,
    model: &str,
) -> (
    crate::handler::dashboard::providers::ProviderProbeCheck,
    crate::handler::dashboard::providers::ProviderProbeCheck,
) {
    let resolved_base_url = auth.resolved_base_url();
    let base = normalize_base_url(&resolved_base_url);
    let payload = json!({
        "model": model,
        "max_tokens": 4,
        "messages": [{ "role": "user", "content": "Reply with exactly ok." }],
    });
    let request = apply_auth_headers(
        client
            .post(format!("{base}/v1/messages"))
            .header("anthropic-version", "2023-06-01")
            .json(&payload),
        auth,
    );
    match request.send().await {
        Ok(response) if response.status().is_success() => (
            probe_check("auth", ProbeStatus::Verified, None),
            probe_check("text", ProbeStatus::Verified, None),
        ),
        Ok(response) if matches!(response.status().as_u16(), 401 | 403) => (
            probe_check(
                "auth",
                ProbeStatus::Failed,
                Some("credential rejected by upstream".to_string()),
            ),
            probe_check(
                "text",
                ProbeStatus::Unknown,
                Some("text probe aborted after authentication failure".to_string()),
            ),
        ),
        Ok(response) => (
            probe_check("auth", ProbeStatus::Verified, None),
            probe_check(
                "text",
                ProbeStatus::Failed,
                Some(format!("upstream returned {}", response.status())),
            ),
        ),
        Err(error) => (
            probe_check(
                "auth",
                ProbeStatus::Unknown,
                Some("text probe failed before upstream confirmed auth".to_string()),
            ),
            probe_check("text", ProbeStatus::Failed, Some(error.to_string())),
        ),
    }
}

async fn run_gemini_text_probe(
    client: &reqwest::Client,
    auth: &prism_core::provider::AuthRecord,
    model: &str,
) -> (
    crate::handler::dashboard::providers::ProviderProbeCheck,
    crate::handler::dashboard::providers::ProviderProbeCheck,
) {
    let resolved_base_url = auth.resolved_base_url();
    let base = normalize_base_url(&resolved_base_url);
    let payload = json!({
        "contents": [{
            "role": "user",
            "parts": [{ "text": "Reply with exactly ok." }]
        }],
        "generationConfig": {
            "maxOutputTokens": 4
        }
    });
    let request = apply_auth_headers(
        client
            .post(format!("{base}/v1beta/models/{model}:generateContent"))
            .json(&payload),
        auth,
    );
    match request.send().await {
        Ok(response) if response.status().is_success() => (
            probe_check("auth", ProbeStatus::Verified, None),
            probe_check("text", ProbeStatus::Verified, None),
        ),
        Ok(response) if matches!(response.status().as_u16(), 401 | 403) => (
            probe_check(
                "auth",
                ProbeStatus::Failed,
                Some("credential rejected by upstream".to_string()),
            ),
            probe_check(
                "text",
                ProbeStatus::Unknown,
                Some("text probe aborted after authentication failure".to_string()),
            ),
        ),
        Ok(response) => (
            probe_check("auth", ProbeStatus::Verified, None),
            probe_check(
                "text",
                ProbeStatus::Failed,
                Some(format!("upstream returned {}", response.status())),
            ),
        ),
        Err(error) => (
            probe_check(
                "auth",
                ProbeStatus::Unknown,
                Some("text probe failed before upstream confirmed auth".to_string()),
            ),
            probe_check("text", ProbeStatus::Failed, Some(error.to_string())),
        ),
    }
}

pub fn cached_probe_result(state: &AppState, provider_name: &str) -> Option<ProviderProbeResult> {
    state
        .provider_probe_cache
        .get(provider_name)
        .map(|entry| entry.value().clone())
}

async fn run_generic_health_probe(
    provider_name: &str,
    auth: &prism_core::provider::AuthRecord,
    client: &reqwest::Client,
) -> ProviderProbeResult {
    let started = Instant::now();
    let model = configured_probe_model(auth);
    let (auth_check, text_check) = match (auth.provider, model) {
        (_, None) => (
            probe_check(
                "auth",
                ProbeStatus::Unknown,
                Some("no configured model available for live auth probe".to_string()),
            ),
            probe_check(
                "text",
                ProbeStatus::Unknown,
                Some("no configured model available for live text probe".to_string()),
            ),
        ),
        (prism_core::provider::Format::OpenAI, Some(model)) => {
            run_openai_text_probe(client, auth, model).await
        }
        (prism_core::provider::Format::Claude, Some(model)) => {
            run_claude_text_probe(client, auth, model).await
        }
        (prism_core::provider::Format::Gemini, Some(model)) => {
            run_gemini_text_probe(client, auth, model).await
        }
    };
    let status = summarize_generic_status(&auth_check, &text_check);

    ProviderProbeResult {
        provider: provider_name.to_string(),
        upstream: auth.upstream.to_string(),
        status: status.to_string(),
        checked_at: chrono::Utc::now().to_rfc3339(),
        latency_ms: started.elapsed().as_millis() as u64,
        checks: vec![
            auth_check,
            text_check,
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

pub async fn health_check(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let provider_name = match provider_name_from_config(&state, &name) {
        Ok(provider_name) => provider_name,
        Err(response) => return response,
    };

    let Some(auth) = select_runtime_auth(&state, &provider_name) else {
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
        Ok(client) => client,
        Err(error) => return client_error_response(error),
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
