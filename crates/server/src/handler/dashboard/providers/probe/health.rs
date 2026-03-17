use std::time::Instant;

use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde_json::json;

use super::codex::run_codex_probe;
use super::common::{
    build_reqwest_client, client_error_response, probe_check, provider_name_from_config,
};
use super::models::build_models_request;
use super::super::{ProbeStatus, ProviderProbeResult};

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
        Ok(request) => match request.send().await {
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

pub async fn health_check(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let provider_name = match provider_name_from_config(&state, &name) {
        Ok(provider_name) => provider_name,
        Err(response) => return response,
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
