use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use prism_core::auth_profile::{AuthMode, AuthProfileEntry, OAuthTokenState};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Serialize)]
struct ProviderSummary {
    name: String,
    format: String,
    api_key_masked: String,
    base_url: Option<String>,
    models: Vec<prism_core::config::ModelMapping>,
    disabled: bool,
    wire_api: prism_core::provider::WireApi,
    upstream_presentation: prism_core::presentation::UpstreamPresentationConfig,
    auth_profiles: Vec<AuthProfileSummary>,
}

#[derive(Debug, Deserialize)]
pub struct CreateProviderRequest {
    pub name: String,
    pub format: String,
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

fn strip_runtime_oauth_data(
    profiles: Vec<AuthProfileEntry>,
) -> (Vec<AuthProfileEntry>, Vec<(String, OAuthTokenState)>) {
    let mut stripped = Vec::with_capacity(profiles.len());
    let mut runtime_states = Vec::new();

    for mut profile in profiles {
        if profile.mode == AuthMode::OpenaiCodexOauth
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
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "error": "validation_failed",
                "message": "api_key and auth_profiles are mutually exclusive"
            })),
        ));
    }
    Ok(())
}

fn is_valid_format(format_str: &str) -> bool {
    matches!(format_str, "openai" | "claude" | "gemini")
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
    let auth_profiles = match normalize_auth_profiles(&body.auth_profiles) {
        Ok(profiles) => profiles,
        Err(response) => return response,
    };
    if let Err(response) = validate_auth_shape(body.api_key.as_deref(), &auth_profiles) {
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

    let format: prism_core::provider::Format = body
        .format
        .parse()
        .unwrap_or(prism_core::provider::Format::OpenAI);

    let models = body
        .models
        .into_iter()
        .map(|id| prism_core::config::ModelMapping { id, alias: None })
        .collect();

    let wire_api = match body.wire_api.as_deref() {
        Some("responses") => prism_core::provider::WireApi::Responses,
        _ => prism_core::provider::WireApi::Chat,
    };

    let provider_name = body.name.clone();
    let api_key = body.api_key.unwrap_or_default();
    let (auth_profiles, runtime_oauth_states) = strip_runtime_oauth_data(auth_profiles);

    let new_entry = prism_core::config::ProviderKeyEntry {
        name: provider_name.clone(),
        format,
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
                error = %e,
                "Failed to create provider"
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "write_failed", "message": e})),
            )
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
            if let Some(ref wire_api_opt) = body.wire_api {
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
            tracing::error!(provider = %name_for_log, error = %e, "Failed to update provider");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "write_failed", "message": e})),
            )
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
            tracing::error!(provider = %name_for_log, error = %e, "Failed to delete provider");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "write_failed", "message": e})),
            )
        }
    }
}

async fn update_config_file(
    state: &AppState,
    mutate: impl FnOnce(&mut prism_core::config::Config),
) -> Result<(), String> {
    super::config_tx::update_config_versioned(state, None, mutate)
        .await
        .map(|_| ())
        .map_err(|e| match e {
            super::config_tx::ConfigTxError::Conflict { .. } => "conflict".to_string(),
            super::config_tx::ConfigTxError::Validation(msg)
            | super::config_tx::ConfigTxError::Internal(msg) => msg,
        })
}

#[derive(Debug, Deserialize)]
pub struct FetchModelsRequest {
    pub format: String,
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

fn default_base_url(provider_type: &str) -> Option<&'static str> {
    match provider_type {
        "openai" => Some("https://api.openai.com"),
        "claude" => Some("https://api.anthropic.com"),
        "gemini" => Some("https://generativelanguage.googleapis.com"),
        _ => None,
    }
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

    // Resolve base URL
    let base_url = match body.base_url.as_deref().filter(|s| !s.is_empty()) {
        Some(url) => url.to_string(),
        None => match default_base_url(format) {
            Some(url) => url.to_string(),
            None => {
                return (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(
                        json!({"error": "validation_failed", "message": "base_url is required for this provider"}),
                    ),
                );
            }
        },
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

    let ptype = entry.format.as_str();

    // Resolve base URL: entry-level, then default
    let base_url = entry
        .base_url
        .as_deref()
        .filter(|s| !s.is_empty())
        .or_else(|| default_base_url(ptype))
        .unwrap_or("")
        .to_string();

    if base_url.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"status": "error", "message": "No base_url configured for this provider"})),
        );
    }

    // Use entry-level proxy, fall back to global proxy
    let proxy_url = entry.proxy_url.as_deref().or(config.proxy_url.as_deref());

    let client = match build_reqwest_client(&state.http_client_pool, proxy_url, 10) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"status": "error", "message": e})),
            );
        }
    };

    // Try /v1/models first, fallback to a minimal chat completions probe
    let start = Instant::now();
    let models_req = build_models_request(
        &client,
        ptype,
        &entry.api_key,
        &base_url,
        Some(&entry.headers),
    );

    let mut success = false;
    if let Ok(req) = models_req {
        match req.send().await {
            Ok(resp) if resp.status().is_success() => {
                success = true;
            }
            Ok(resp) if resp.status().as_u16() == 404 || resp.status().as_u16() == 405 => {
                // /v1/models not supported, try chat completions probe
            }
            Ok(resp) => {
                let status = resp.status();
                let body_text = resp.text().await.unwrap_or_default();
                let latency_ms = start.elapsed().as_millis() as u64;
                return (
                    StatusCode::OK,
                    Json(
                        json!({"status": "error", "latency_ms": latency_ms, "message": format!("Upstream returned {status}: {body_text}")}),
                    ),
                );
            }
            Err(e) => {
                return (
                    StatusCode::OK,
                    Json(
                        json!({"status": "error", "message": format!("Failed to reach upstream: {e}")}),
                    ),
                );
            }
        }
    }

    // Fallback: send a minimal chat completions request with max_tokens=1
    if !success {
        let base = normalize_base_url(&base_url);
        let chat_url = match ptype {
            "gemini" => {
                // Gemini uses a different endpoint; just report models endpoint unsupported
                let latency_ms = start.elapsed().as_millis() as u64;
                return (
                    StatusCode::OK,
                    Json(
                        json!({"status": "error", "latency_ms": latency_ms, "message": "Models endpoint not available for this provider"}),
                    ),
                );
            }
            "claude" => format!("{base}/v1/messages"),
            _ => format!("{base}/v1/chat/completions"),
        };

        // Send an intentionally invalid request (empty body) to probe connectivity
        // and key validity without consuming tokens.
        // - 400 = reachable, key accepted (just bad params) -> healthy
        // - 401/403 = reachable but key invalid -> report error
        // - 5xx = server error -> report error
        let mut req = client
            .post(&chat_url)
            .header("content-type", "application/json")
            .body("{}");
        // Add auth headers
        match ptype {
            "claude" => {
                req = req
                    .header("x-api-key", &entry.api_key)
                    .header("anthropic-version", "2023-06-01");
            }
            _ => {
                req = req.header("Authorization", format!("Bearer {}", entry.api_key));
            }
        }
        // Add custom headers
        for (k, v) in &entry.headers {
            req = req.header(k.as_str(), v.as_str());
        }

        match req.send().await {
            Ok(resp) => {
                let latency_ms = start.elapsed().as_millis() as u64;
                let status_code = resp.status().as_u16();
                // 400 = reachable & key valid (bad params expected)
                // 401/403 = key invalid
                // 5xx = server error
                match status_code {
                    400 | 422 => {
                        return (
                            StatusCode::OK,
                            Json(json!({"status": "ok", "latency_ms": latency_ms})),
                        );
                    }
                    401 | 403 => {
                        return (
                            StatusCode::OK,
                            Json(
                                json!({"status": "error", "latency_ms": latency_ms, "message": "Authentication failed: invalid API key"}),
                            ),
                        );
                    }
                    _ if status_code < 500 => {
                        return (
                            StatusCode::OK,
                            Json(json!({"status": "ok", "latency_ms": latency_ms})),
                        );
                    }
                    _ => {}
                }
                let body_text = resp.text().await.unwrap_or_default();
                return (
                    StatusCode::OK,
                    Json(
                        json!({"status": "error", "latency_ms": latency_ms, "message": format!("Upstream returned {status_code}: {body_text}")}),
                    ),
                );
            }
            Err(e) => {
                return (
                    StatusCode::OK,
                    Json(
                        json!({"status": "error", "message": format!("Failed to reach upstream: {e}")}),
                    ),
                );
            }
        }
    }

    let latency_ms = start.elapsed().as_millis() as u64;
    (
        StatusCode::OK,
        Json(json!({"status": "ok", "latency_ms": latency_ms})),
    )
}
