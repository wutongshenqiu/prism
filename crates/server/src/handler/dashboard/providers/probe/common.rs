use axum::Json;
use axum::http::StatusCode;
use serde_json::json;

use crate::AppState;

use super::super::{ProbeStatus, ProviderProbeCheck};
use prism_core::auth_profile::AuthHeaderKind;

pub(super) fn build_reqwest_client(
    pool: &prism_core::proxy::HttpClientPool,
    proxy_url: Option<&str>,
    timeout_secs: u64,
) -> Result<reqwest::Client, String> {
    pool.get_or_create(None, proxy_url, timeout_secs, timeout_secs)
        .map_err(|e| format!("Failed to build HTTP client: {e}"))
}

pub(super) fn probe_check(
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

pub(super) fn normalize_base_url(base_url: &str) -> &str {
    let url = base_url.trim_end_matches('/');
    if let Some(stripped) = url.strip_suffix("/v1") {
        stripped
    } else if let Some(stripped) = url.strip_suffix("/v1beta") {
        stripped
    } else {
        url
    }
}

pub(super) fn apply_auth_headers(
    mut request: reqwest::RequestBuilder,
    auth: &prism_core::provider::AuthRecord,
) -> reqwest::RequestBuilder {
    request = match auth.resolved_auth_header_kind() {
        AuthHeaderKind::Bearer => {
            request.header("Authorization", format!("Bearer {}", auth.current_secret()))
        }
        AuthHeaderKind::XApiKey => request.header("x-api-key", auth.current_secret()),
        AuthHeaderKind::XGoogApiKey => request.header("x-goog-api-key", auth.current_secret()),
        AuthHeaderKind::Auto => request,
    };

    for (key, value) in &auth.headers {
        request = request.header(key.as_str(), value.as_str());
    }

    request
}

pub(super) fn select_runtime_auth(
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

pub(super) fn provider_not_found_response() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(json!({"error": "not_found", "message": "Provider not found"})),
    )
}

pub(super) fn client_error_response(message: String) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": "client_error", "message": message})),
    )
}

pub(super) fn provider_name_from_config(
    state: &AppState,
    name: &str,
) -> Result<String, (StatusCode, Json<serde_json::Value>)> {
    let config = state.config.load();
    config
        .providers
        .iter()
        .find(|entry| entry.name == name)
        .map(|entry| entry.name.clone())
        .ok_or_else(provider_not_found_response)
}
