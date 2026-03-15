use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use prism_core::auth_profile::{AuthProfileEntry, OAuthTokenState};
use prism_provider::sse::parse_sse_stream;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::time::Instant;
use tokio_stream::StreamExt;

#[derive(Debug, Serialize)]
struct ProviderSummary {
    name: String,
    format: String,
    upstream: String,
    api_key_masked: String,
    base_url: Option<String>,
    models: Vec<prism_core::config::ModelMapping>,
    disabled: bool,
    wire_api: prism_core::provider::WireApi,
    upstream_presentation: prism_core::presentation::UpstreamPresentationConfig,
    auth_profiles: Vec<AuthProfileSummary>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProbeStatus {
    Verified,
    Failed,
    Unknown,
    Unsupported,
}

impl ProbeStatus {
    fn is_verified(self) -> bool {
        matches!(self, Self::Verified)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderProbeCheck {
    pub capability: String,
    pub status: ProbeStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderProbeResult {
    pub provider: String,
    pub upstream: String,
    pub status: String,
    pub checked_at: String,
    pub latency_ms: u64,
    pub checks: Vec<ProviderProbeCheck>,
}

impl ProviderProbeResult {
    pub fn capability_status(&self, capability: &str) -> ProbeStatus {
        self.checks
            .iter()
            .find(|check| check.capability == capability)
            .map(|check| check.status)
            .unwrap_or(ProbeStatus::Unknown)
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateProviderRequest {
    pub name: String,
    pub format: String,
    #[serde(default)]
    pub upstream: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub auth_profiles: Vec<AuthProfileEntry>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub proxy_url: Option<String>,
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub excluded_models: Vec<String>,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub wire_api: Option<String>,
    #[serde(default = "default_weight")]
    pub weight: u32,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub upstream_presentation: Option<prism_core::presentation::UpstreamPresentationConfig>,
    #[serde(default)]
    pub vertex: bool,
    #[serde(default)]
    pub vertex_project: Option<String>,
    #[serde(default)]
    pub vertex_location: Option<String>,
}

fn default_weight() -> u32 {
    1
}

#[derive(Debug, Deserialize)]
pub struct UpdateProviderRequest {
    #[serde(default)]
    pub upstream: Option<Option<String>>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub auth_profiles: Option<Vec<AuthProfileEntry>>,
    #[serde(default)]
    pub base_url: Option<Option<String>>,
    #[serde(default)]
    pub proxy_url: Option<Option<String>>,
    #[serde(default)]
    pub prefix: Option<Option<String>>,
    #[serde(default)]
    pub models: Option<Vec<String>>,
    #[serde(default)]
    pub excluded_models: Option<Vec<String>>,
    #[serde(default)]
    pub headers: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    pub disabled: Option<bool>,
    #[serde(default)]
    pub wire_api: Option<Option<String>>,
    #[serde(default)]
    pub weight: Option<u32>,
    #[serde(default)]
    pub region: Option<Option<String>>,
    #[serde(default)]
    pub upstream_presentation: Option<Option<prism_core::presentation::UpstreamPresentationConfig>>,
    #[serde(default)]
    pub vertex: Option<bool>,
    #[serde(default)]
    pub vertex_project: Option<Option<String>>,
    #[serde(default)]
    pub vertex_location: Option<Option<String>>,
}

#[derive(Debug, Serialize)]
struct AuthProfileSummary {
    id: String,
    qualified_name: String,
    mode: prism_core::auth_profile::AuthMode,
    header: prism_core::auth_profile::AuthHeaderKind,
    secret_masked: Option<String>,
    access_token_masked: Option<String>,
    refresh_token_present: bool,
    id_token_present: bool,
    expires_at: Option<String>,
    account_id: Option<String>,
    email: Option<String>,
    last_refresh: Option<String>,
    headers: HashMap<String, String>,
    disabled: bool,
    weight: u32,
    region: Option<String>,
    prefix: Option<String>,
    upstream_presentation: prism_core::presentation::UpstreamPresentationConfig,
}

fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        return "****".to_string();
    }
    format!("{}****{}", &key[..4], &key[key.len() - 4..])
}

fn mask_optional_key(key: Option<&str>) -> Option<String> {
    key.filter(|value| !value.is_empty()).map(mask_key)
}

fn provider_api_key_masked(
    state: &AppState,
    entry: &prism_core::config::ProviderKeyEntry,
) -> String {
    if !entry.api_key.is_empty() {
        return mask_key(&entry.api_key);
    }

    entry
        .expanded_auth_profiles()
        .into_iter()
        .find_map(|profile| {
            let hydrated = state
                .auth_runtime
                .apply_runtime_state(&entry.name, &profile)
                .unwrap_or(profile);
            hydrated
                .secret
                .as_deref()
                .filter(|value| !value.is_empty())
                .or_else(|| {
                    hydrated
                        .access_token
                        .as_deref()
                        .filter(|value| !value.is_empty())
                })
                .map(mask_key)
        })
        .unwrap_or_default()
}

fn summarize_auth_profile(provider_name: &str, profile: &AuthProfileEntry) -> AuthProfileSummary {
    AuthProfileSummary {
        id: profile.id.clone(),
        qualified_name: format!("{provider_name}/{}", profile.id),
        mode: profile.mode,
        header: profile.header,
        secret_masked: mask_optional_key(profile.secret.as_deref()),
        access_token_masked: mask_optional_key(profile.access_token.as_deref()),
        refresh_token_present: profile
            .refresh_token
            .as_deref()
            .is_some_and(|value| !value.is_empty()),
        id_token_present: profile
            .id_token
            .as_deref()
            .is_some_and(|value| !value.is_empty()),
        expires_at: profile.expires_at.clone(),
        account_id: profile.account_id.clone(),
        email: profile.email.clone(),
        last_refresh: profile.last_refresh.clone(),
        headers: profile.headers.clone(),
        disabled: profile.disabled,
        weight: profile.weight.max(1),
        region: profile.region.clone(),
        prefix: profile.prefix.clone(),
        upstream_presentation: profile.upstream_presentation.clone(),
    }
}

fn summarize_provider(
    state: &AppState,
    entry: &prism_core::config::ProviderKeyEntry,
) -> ProviderSummary {
    let auth_profiles = entry
        .expanded_auth_profiles()
        .into_iter()
        .map(|profile| {
            let hydrated = state
                .auth_runtime
                .apply_runtime_state(&entry.name, &profile)
                .unwrap_or(profile);
            summarize_auth_profile(&entry.name, &hydrated)
        })
        .collect();

    ProviderSummary {
        name: entry.name.clone(),
        format: entry.format.as_str().to_string(),
        upstream: entry.upstream_kind().as_str().to_string(),
        api_key_masked: provider_api_key_masked(state, entry),
        base_url: entry.base_url.clone(),
        models: entry.models.clone(),
        disabled: entry.disabled,
        wire_api: entry.wire_api,
        upstream_presentation: entry.upstream_presentation.clone(),
        auth_profiles,
    }
}

fn normalize_auth_profiles(
    profiles: &[AuthProfileEntry],
) -> Result<Vec<AuthProfileEntry>, (StatusCode, Json<serde_json::Value>)> {
    let mut normalized = profiles.to_vec();
    for profile in &mut normalized {
        profile.normalize();
        if let Err(err) = profile.validate() {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({"error": "validation_failed", "message": err})),
            ));
        }
    }
    Ok(normalized)
}

fn validation_error(message: impl Into<String>) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({"error": "validation_failed", "message": message.into()})),
    )
}

fn strip_runtime_oauth_data(
    profiles: Vec<AuthProfileEntry>,
) -> (Vec<AuthProfileEntry>, Vec<(String, OAuthTokenState)>) {
    let mut stripped = Vec::with_capacity(profiles.len());
    let mut runtime_states = Vec::new();

    for mut profile in profiles {
        if profile.mode.is_managed()
            && let Some(state) = OAuthTokenState::from_profile(&profile)
        {
            let has_runtime_material = !state.access_token.is_empty()
                || !state.refresh_token.is_empty()
                || state.id_token.is_some()
                || state.account_id.is_some()
                || state.email.is_some()
                || state.expires_at.is_some()
                || state.last_refresh.is_some();
            if has_runtime_material {
                runtime_states.push((profile.id.clone(), state));
            }
            profile.access_token = None;
            profile.refresh_token = None;
            profile.id_token = None;
            profile.expires_at = None;
            profile.account_id = None;
            profile.email = None;
            profile.last_refresh = None;
        }
        stripped.push(profile);
    }

    (stripped, runtime_states)
}

fn seed_runtime_oauth_states(
    state: &AppState,
    provider_name: &str,
    runtime_states: &[(String, OAuthTokenState)],
) -> Result<(), String> {
    if runtime_states.is_empty() {
        return Ok(());
    }

    for (profile_id, oauth_state) in runtime_states {
        state
            .auth_runtime
            .store_state(provider_name, profile_id, oauth_state.clone())?;
    }

    let config = state.config.load();
    state
        .router
        .set_oauth_states(state.auth_runtime.oauth_snapshot());
    state.router.update_from_config(&config);
    state
        .catalog
        .update_from_credentials(&state.router.credential_map());
    Ok(())
}

fn validate_auth_shape(
    api_key: Option<&str>,
    auth_profiles: &[AuthProfileEntry],
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    let has_api_key = api_key.is_some_and(|value| !value.trim().is_empty());
    let has_profiles = !auth_profiles.is_empty();
    if has_api_key && has_profiles {
        return Err(validation_error(
            "api_key and auth_profiles are mutually exclusive",
        ));
    }
    Ok(())
}

fn validate_provider_auth_profiles(
    format: prism_core::provider::Format,
    upstream: prism_core::provider::UpstreamKind,
    base_url: Option<&str>,
    auth_profiles: &[AuthProfileEntry],
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    for profile in auth_profiles {
        if let Err(message) = profile.validate_for_provider(format, upstream, base_url) {
            return Err(validation_error(message));
        }
    }
    Ok(())
}

fn config_tx_error_response(
    error: super::config_tx::ConfigTxError,
) -> (StatusCode, Json<serde_json::Value>) {
    match error {
        super::config_tx::ConfigTxError::Conflict { current_version } => (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "conflict",
                "message": "config version conflict",
                "current_version": current_version
            })),
        ),
        super::config_tx::ConfigTxError::Validation(message) => validation_error(message),
        super::config_tx::ConfigTxError::Internal(message) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "write_failed", "message": message})),
        ),
    }
}

fn is_valid_format(format_str: &str) -> bool {
    matches!(format_str, "openai" | "claude" | "gemini")
}

fn parse_upstream_kind(
    format: prism_core::provider::Format,
    upstream: Option<&str>,
) -> Result<prism_core::provider::UpstreamKind, (StatusCode, Json<serde_json::Value>)> {
    let Some(raw) = upstream.filter(|value| !value.trim().is_empty()) else {
        return Ok(format.into());
    };
    raw.parse().map_err(validation_error)
}

/// GET /api/dashboard/providers
pub async fn list_providers(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.load();
    let mut providers = Vec::new();

    for entry in config.providers.iter() {
        providers.push(summarize_provider(&state, entry));
    }

    (StatusCode::OK, Json(json!({ "providers": providers })))
}

/// GET /api/dashboard/providers/:name
pub async fn get_provider(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let config = state.config.load();

    match config.providers.iter().find(|e| e.name == name) {
        Some(entry) => {
            let detail = json!({
                "name": entry.name,
                "format": entry.format.as_str(),
                "upstream": entry.upstream_kind().as_str(),
                "api_key_masked": provider_api_key_masked(&state, entry),
                "base_url": entry.base_url,
                "proxy_url": entry.proxy_url,
                "prefix": entry.prefix,
                "models": entry.models,
                "excluded_models": entry.excluded_models,
                "headers": entry.headers,
                "disabled": entry.disabled,
                "wire_api": entry.wire_api,
                "weight": entry.weight,
                "region": entry.region,
                "upstream_presentation": entry.upstream_presentation,
                "vertex": entry.vertex,
                "vertex_project": entry.vertex_project,
                "vertex_location": entry.vertex_location,
                "auth_profiles": entry
                    .expanded_auth_profiles()
                    .into_iter()
                    .map(|profile| {
                        let hydrated = state
                            .auth_runtime
                            .apply_runtime_state(&entry.name, &profile)
                            .unwrap_or(profile);
                        summarize_auth_profile(&entry.name, &hydrated)
                    })
                    .collect::<Vec<_>>(),
            });
            (StatusCode::OK, Json(detail))
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "not_found", "message": "Provider not found"})),
        ),
    }
}

/// POST /api/dashboard/providers
pub async fn create_provider(
    State(state): State<AppState>,
    Json(body): Json<CreateProviderRequest>,
) -> impl IntoResponse {
    if body.name.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": "validation_failed", "message": "name is required"})),
        );
    }
    if !is_valid_format(&body.format) {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(
                json!({"error": "validation_failed", "message": "Invalid format. Must be one of: openai, claude, gemini"}),
            ),
        );
    }
    let format: prism_core::provider::Format = body
        .format
        .parse()
        .unwrap_or(prism_core::provider::Format::OpenAI);
    let upstream = match parse_upstream_kind(format, body.upstream.as_deref()) {
        Ok(value) => value,
        Err(response) => return response,
    };

    let auth_profiles = match normalize_auth_profiles(&body.auth_profiles) {
        Ok(profiles) => profiles,
        Err(response) => return response,
    };
    if let Err(response) = validate_auth_shape(body.api_key.as_deref(), &auth_profiles) {
        return response;
    }
    if let Err(response) =
        validate_provider_auth_profiles(format, upstream, body.base_url.as_deref(), &auth_profiles)
    {
        return response;
    }

    // Check name uniqueness
    {
        let config = state.config.load();
        if config.providers.iter().any(|e| e.name == body.name) {
            return (
                StatusCode::CONFLICT,
                Json(
                    json!({"error": "duplicate_name", "message": format!("Provider name '{}' already exists", body.name)}),
                ),
            );
        }
    }

    let models = body
        .models
        .into_iter()
        .map(|id| prism_core::config::ModelMapping { id, alias: None })
        .collect();

    let wire_api = if upstream == prism_core::provider::UpstreamKind::Codex {
        prism_core::provider::WireApi::Responses
    } else {
        match body.wire_api.as_deref() {
            Some("responses") => prism_core::provider::WireApi::Responses,
            _ => prism_core::provider::WireApi::Chat,
        }
    };

    let provider_name = body.name.clone();
    let api_key = body.api_key.unwrap_or_default();
    let (auth_profiles, runtime_oauth_states) = strip_runtime_oauth_data(auth_profiles);

    let new_entry = prism_core::config::ProviderKeyEntry {
        name: provider_name.clone(),
        format,
        upstream: Some(upstream),
        api_key,
        base_url: body.base_url,
        proxy_url: body.proxy_url,
        prefix: body.prefix,
        models,
        excluded_models: body.excluded_models,
        headers: body.headers,
        disabled: body.disabled,
        cloak: Default::default(),
        upstream_presentation: body.upstream_presentation.unwrap_or_default(),
        wire_api,
        weight: body.weight,
        region: body.region,
        credential_source: None,
        auth_profiles,
        vertex: body.vertex,
        vertex_project: body.vertex_project,
        vertex_location: body.vertex_location,
    };
    if let Err(message) = new_entry.validate_shape() {
        return validation_error(message);
    }

    match update_config_file(&state, |config| {
        config.providers.push(new_entry.clone());
    })
    .await
    {
        Ok(()) => {
            if let Err(err) =
                seed_runtime_oauth_states(&state, &provider_name, &runtime_oauth_states)
            {
                tracing::error!(
                    name = %provider_name,
                    error = %err,
                    "Provider created but runtime oauth seeding failed"
                );
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "runtime_auth_seed_failed", "message": err})),
                );
            }
            tracing::info!(
                name = %provider_name,
                format = %body.format,
                "Provider created via dashboard"
            );
            (
                StatusCode::CREATED,
                Json(json!({"message": "Provider created successfully"})),
            )
        }
        Err(e) => {
            tracing::error!(
                name = %provider_name,
                error = ?e,
                "Failed to create provider"
            );
            config_tx_error_response(e)
        }
    }
}

/// PATCH /api/dashboard/providers/:name
pub async fn update_provider(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(body): Json<UpdateProviderRequest>,
) -> impl IntoResponse {
    // Verify provider exists
    let existing_entry = {
        let config = state.config.load();
        match config.providers.iter().find(|e| e.name == name) {
            Some(entry) => entry.clone(),
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": "not_found", "message": "Provider not found"})),
                );
            }
        }
    };

    let name_for_log = name.clone();
    let auth_profiles = match body
        .auth_profiles
        .as_ref()
        .map(|profiles| normalize_auth_profiles(profiles))
        .transpose()
    {
        Ok(profiles) => profiles,
        Err(response) => return response,
    };
    if let Some(ref profiles) = auth_profiles
        && let Err(response) = validate_auth_shape(body.api_key.as_deref(), profiles)
    {
        return response;
    }
    let upstream = match parse_upstream_kind(
        existing_entry.format,
        body.upstream.as_ref().and_then(|value| value.as_deref()),
    ) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let mut candidate_entry = existing_entry.clone();
    candidate_entry.upstream = Some(upstream);
    if let Some(ref key) = body.api_key {
        candidate_entry.api_key = key.clone();
    }
    if let Some(ref profiles) = auth_profiles {
        candidate_entry.auth_profiles = profiles.clone();
        if !profiles.is_empty() && body.api_key.is_none() {
            candidate_entry.api_key.clear();
        }
    }
    if let Some(ref url) = body.base_url {
        candidate_entry.base_url = url.clone();
    }
    if let Some(ref url) = body.proxy_url {
        candidate_entry.proxy_url = url.clone();
    }
    if let Some(ref prefix) = body.prefix {
        candidate_entry.prefix = prefix.clone();
    }
    if let Some(ref models) = body.models {
        candidate_entry.models = models
            .iter()
            .map(|id| prism_core::config::ModelMapping {
                id: id.clone(),
                alias: None,
            })
            .collect();
    }
    if let Some(ref excluded) = body.excluded_models {
        candidate_entry.excluded_models = excluded.clone();
    }
    if let Some(ref headers) = body.headers {
        candidate_entry.headers = headers.clone();
    }
    if let Some(disabled) = body.disabled {
        candidate_entry.disabled = disabled;
    }
    if upstream == prism_core::provider::UpstreamKind::Codex {
        candidate_entry.wire_api = prism_core::provider::WireApi::Responses;
    } else if let Some(ref wire_api_opt) = body.wire_api {
        candidate_entry.wire_api = match wire_api_opt.as_deref() {
            Some("responses") => prism_core::provider::WireApi::Responses,
            _ => prism_core::provider::WireApi::Chat,
        };
    }
    if let Some(weight) = body.weight {
        candidate_entry.weight = weight;
    }
    if let Some(ref region) = body.region {
        candidate_entry.region = region.clone();
    }
    if let Some(ref presentation_opt) = body.upstream_presentation {
        candidate_entry.upstream_presentation = presentation_opt.clone().unwrap_or_default();
    }
    if let Some(vertex) = body.vertex {
        candidate_entry.vertex = vertex;
    }
    if let Some(ref project) = body.vertex_project {
        candidate_entry.vertex_project = project.clone();
    }
    if let Some(ref location) = body.vertex_location {
        candidate_entry.vertex_location = location.clone();
    }

    if let Err(response) = validate_auth_shape(
        Some(candidate_entry.api_key.as_str()),
        &candidate_entry.auth_profiles,
    ) {
        return response;
    }
    if let Err(response) = validate_provider_auth_profiles(
        candidate_entry.format,
        candidate_entry.upstream_kind(),
        candidate_entry.base_url.as_deref(),
        &candidate_entry.auth_profiles,
    ) {
        return response;
    }
    if let Err(message) = candidate_entry.validate_shape() {
        return validation_error(message);
    }
    let runtime_oauth_states = auth_profiles.clone().map(strip_runtime_oauth_data);
    let auth_profiles_for_write = runtime_oauth_states
        .as_ref()
        .map(|(profiles, _)| profiles.clone());
    let runtime_oauth_states = runtime_oauth_states
        .map(|(_, states)| states)
        .unwrap_or_default();

    match update_config_file(&state, move |config| {
        if let Some(entry) = config.providers.iter_mut().find(|e| e.name == name) {
            if let Some(ref key) = body.api_key {
                entry.api_key = key.clone();
            }
            if let Some(ref upstream_opt) = body.upstream {
                entry.upstream = upstream_opt
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
                    .and_then(|value| value.parse().ok());
            }
            if let Some(ref profiles) = auth_profiles_for_write {
                entry.auth_profiles = profiles.clone();
                if !profiles.is_empty() && body.api_key.is_none() {
                    entry.api_key.clear();
                }
            }
            if let Some(ref url) = body.base_url {
                entry.base_url = url.clone();
            }
            if let Some(ref url) = body.proxy_url {
                entry.proxy_url = url.clone();
            }
            if let Some(ref prefix) = body.prefix {
                entry.prefix = prefix.clone();
            }
            if let Some(ref models) = body.models {
                entry.models = models
                    .iter()
                    .map(|id| prism_core::config::ModelMapping {
                        id: id.clone(),
                        alias: None,
                    })
                    .collect();
            }
            if let Some(ref excluded) = body.excluded_models {
                entry.excluded_models = excluded.clone();
            }
            if let Some(ref headers) = body.headers {
                entry.headers = headers.clone();
            }
            if let Some(disabled) = body.disabled {
                entry.disabled = disabled;
            }
            if entry.upstream_kind() == prism_core::provider::UpstreamKind::Codex {
                entry.wire_api = prism_core::provider::WireApi::Responses;
            } else if let Some(ref wire_api_opt) = body.wire_api {
                entry.wire_api = match wire_api_opt.as_deref() {
                    Some("responses") => prism_core::provider::WireApi::Responses,
                    _ => prism_core::provider::WireApi::Chat,
                };
            }
            if let Some(weight) = body.weight {
                entry.weight = weight;
            }
            if let Some(ref region) = body.region {
                entry.region = region.clone();
            }
            if let Some(ref presentation_opt) = body.upstream_presentation {
                entry.upstream_presentation = presentation_opt.clone().unwrap_or_default();
            }
            if let Some(vertex) = body.vertex {
                entry.vertex = vertex;
            }
            if let Some(ref project) = body.vertex_project {
                entry.vertex_project = project.clone();
            }
            if let Some(ref location) = body.vertex_location {
                entry.vertex_location = location.clone();
            }
        }
    })
    .await
    {
        Ok(()) => {
            if let Err(err) =
                seed_runtime_oauth_states(&state, &name_for_log, &runtime_oauth_states)
            {
                tracing::error!(
                    provider = %name_for_log,
                    error = %err,
                    "Provider updated but runtime oauth seeding failed"
                );
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "runtime_auth_seed_failed", "message": err})),
                );
            }
            tracing::info!(provider = %name_for_log, "Provider updated via dashboard");
            (
                StatusCode::OK,
                Json(json!({"message": "Provider updated successfully"})),
            )
        }
        Err(e) => {
            tracing::error!(provider = %name_for_log, error = ?e, "Failed to update provider");
            config_tx_error_response(e)
        }
    }
}

/// DELETE /api/dashboard/providers/:name
pub async fn delete_provider(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    // Verify provider exists
    {
        let config = state.config.load();
        if !config.providers.iter().any(|e| e.name == name) {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "not_found", "message": "Provider not found"})),
            );
        }
    }

    let name_for_log = name.clone();
    match update_config_file(&state, move |config| {
        config.providers.retain(|e| e.name != name);
    })
    .await
    {
        Ok(()) => {
            tracing::info!(provider = %name_for_log, "Provider deleted via dashboard");
            (
                StatusCode::OK,
                Json(json!({"message": "Provider deleted successfully"})),
            )
        }
        Err(e) => {
            tracing::error!(provider = %name_for_log, error = ?e, "Failed to delete provider");
            config_tx_error_response(e)
        }
    }
}

async fn update_config_file(
    state: &AppState,
    mutate: impl FnOnce(&mut prism_core::config::Config),
) -> Result<(), super::config_tx::ConfigTxError> {
    super::config_tx::update_config_versioned(state, None, mutate)
        .await
        .map(|_| ())
}

#[derive(Debug, Deserialize)]
pub struct FetchModelsRequest {
    pub format: String,
    #[serde(default)]
    pub upstream: Option<String>,
    pub api_key: String,
    #[serde(default)]
    pub base_url: Option<String>,
}

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

/// Strip trailing slash and known version prefixes (/v1, /v1beta) from a base URL.
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

const CODEX_HEALTH_USER_AGENT: &str =
    "codex_cli_rs/0.101.0 (Mac OS 26.0.1; arm64) Apple_Terminal/464";
const RED_DOT_PNG_DATA_URL: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC";

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

/// POST /api/dashboard/providers/fetch-models
pub async fn fetch_models(
    State(state): State<AppState>,
    Json(body): Json<FetchModelsRequest>,
) -> impl IntoResponse {
    let format = body.format.as_str();

    // Validate format
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

    // Resolve base URL
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

/// POST /api/dashboard/providers/{name}/presentation-preview
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

#[derive(Debug, Deserialize)]
pub struct PresentationPreviewRequest {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub user_agent: Option<String>,
    #[serde(default)]
    pub sample_body: Option<serde_json::Value>,
}

/// POST /api/dashboard/providers/{name}/health
pub async fn health_check(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let config = state.config.load();

    let entry = match config.providers.iter().find(|e| e.name == name) {
        Some(e) => e,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"status": "error", "message": "Provider not found"})),
            );
        }
    };

    let Some(auth) = select_health_auth(&state, &entry.name) else {
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

    let proxy_url = auth.effective_proxy(config.proxy_url.as_deref());

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
        run_generic_health_probe(&entry.name, &auth, &client).await
    };
    state
        .provider_probe_cache
        .insert(entry.name.clone(), result.clone());

    (StatusCode::OK, Json(json!(result)))
}
