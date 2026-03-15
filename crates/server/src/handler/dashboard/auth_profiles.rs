use super::config_tx::{ConfigTxError, update_config_versioned};
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use chrono::{Duration, Utc};
use prism_core::auth_profile::{
    AuthHeaderKind, AuthMode, AuthProfileEntry, OAuthTokenState,
    validate_anthropic_subscription_token,
};
use prism_core::presentation::UpstreamPresentationConfig;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

const OAUTH_SESSION_TTL_MINUTES: i64 = 10;
const DEVICE_SESSION_TTL_MINUTES: i64 = 15;

#[derive(Debug, Serialize)]
struct AuthProfileListItem {
    provider: String,
    format: String,
    id: String,
    qualified_name: String,
    mode: AuthMode,
    header: AuthHeaderKind,
    connected: bool,
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
    upstream_presentation: UpstreamPresentationConfig,
}

#[derive(Debug, Deserialize)]
pub struct CreateAuthProfileRequest {
    pub provider: String,
    pub id: String,
    pub mode: AuthMode,
    #[serde(default)]
    pub header: AuthHeaderKind,
    #[serde(default)]
    pub secret: Option<String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default = "default_weight")]
    pub weight: u32,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default)]
    pub upstream_presentation: UpstreamPresentationConfig,
}

#[derive(Debug, Deserialize)]
pub struct ReplaceAuthProfileRequest {
    pub mode: AuthMode,
    #[serde(default)]
    pub header: AuthHeaderKind,
    #[serde(default)]
    pub secret: Option<String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default = "default_weight")]
    pub weight: u32,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default)]
    pub upstream_presentation: UpstreamPresentationConfig,
}

#[derive(Debug, Deserialize)]
pub struct StartCodexOauthRequest {
    pub provider: String,
    pub profile_id: String,
    pub redirect_uri: String,
}

#[derive(Debug, Deserialize)]
pub struct CompleteCodexOauthRequest {
    pub state: String,
    pub code: String,
}

#[derive(Debug, Deserialize)]
pub struct StartCodexDeviceRequest {
    pub provider: String,
    pub profile_id: String,
}

#[derive(Debug, Deserialize)]
pub struct PollCodexDeviceRequest {
    pub state: String,
}

#[derive(Debug, Deserialize)]
pub struct ConnectAuthProfileRequest {
    pub secret: String,
}

struct AuthProfileDraft {
    mode: AuthMode,
    header: AuthHeaderKind,
    secret: Option<String>,
    headers: HashMap<String, String>,
    disabled: bool,
    weight: u32,
    region: Option<String>,
    prefix: Option<String>,
    upstream_presentation: UpstreamPresentationConfig,
}

fn default_weight() -> u32 {
    1
}

fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        return "****".to_string();
    }
    format!("{}****{}", &key[..4], &key[key.len() - 4..])
}

fn mask_optional(value: Option<&str>) -> Option<String> {
    value.filter(|value| !value.is_empty()).map(mask_key)
}

fn profile_connected(profile: &AuthProfileEntry) -> bool {
    match profile.mode {
        AuthMode::ApiKey | AuthMode::BearerToken => profile
            .secret
            .as_deref()
            .is_some_and(|value| !value.is_empty()),
        AuthMode::CodexOAuth | AuthMode::AnthropicClaudeSubscription => {
            profile
                .refresh_token
                .as_deref()
                .is_some_and(|value| !value.is_empty())
                || profile
                    .access_token
                    .as_deref()
                    .is_some_and(|value| !value.is_empty())
        }
    }
}

fn migrate_legacy_provider_auth(entry: &mut prism_core::config::ProviderKeyEntry) {
    if !entry.auth_profiles.is_empty() || entry.api_key.trim().is_empty() {
        return;
    }

    entry.auth_profiles.push(AuthProfileEntry {
        id: entry.name.clone(),
        mode: AuthMode::ApiKey,
        header: AuthHeaderKind::Auto,
        secret: Some(entry.api_key.clone()),
        weight: entry.weight.max(1),
        ..Default::default()
    });
    entry.api_key.clear();
}

fn summarize_profile(
    provider_name: &str,
    format: prism_core::provider::Format,
    profile: &AuthProfileEntry,
) -> AuthProfileListItem {
    AuthProfileListItem {
        provider: provider_name.to_string(),
        format: format.as_str().to_string(),
        id: profile.id.clone(),
        qualified_name: format!("{provider_name}/{}", profile.id),
        mode: profile.mode,
        header: profile.header,
        connected: profile_connected(profile),
        secret_masked: mask_optional(profile.secret.as_deref()),
        access_token_masked: mask_optional(profile.access_token.as_deref()),
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

fn hydrate_profile(
    state: &AppState,
    provider_name: &str,
    profile: &AuthProfileEntry,
) -> Result<AuthProfileEntry, (StatusCode, Json<serde_json::Value>)> {
    state
        .auth_runtime
        .apply_runtime_state(provider_name, profile)
        .map_err(internal_error)
}

fn explicit_profile<'a>(
    config: &'a prism_core::config::Config,
    provider: &str,
    profile_id: &str,
) -> Option<(
    &'a prism_core::config::ProviderKeyEntry,
    &'a AuthProfileEntry,
)> {
    let entry = config
        .providers
        .iter()
        .find(|entry| entry.name == provider)?;
    let profile = entry
        .auth_profiles
        .iter()
        .find(|profile| profile.id == profile_id)?;
    Some((entry, profile))
}

fn current_profile_response(
    state: &AppState,
    provider: &str,
    profile_id: &str,
) -> Result<AuthProfileListItem, (StatusCode, Json<serde_json::Value>)> {
    let config = state.config.load();
    let Some((entry, profile)) = explicit_profile(&config, provider, profile_id) else {
        return Err(not_found("Auth profile not found"));
    };
    let hydrated = hydrate_profile(state, &entry.name, profile)?;
    Ok(summarize_profile(&entry.name, entry.format, &hydrated))
}

fn auth_profile_entry_from_create(
    request: &CreateAuthProfileRequest,
) -> Result<AuthProfileEntry, (StatusCode, Json<serde_json::Value>)> {
    auth_profile_entry(
        &request.id,
        AuthProfileDraft {
            mode: request.mode,
            header: request.header,
            secret: request.secret.clone(),
            headers: request.headers.clone(),
            disabled: request.disabled,
            weight: request.weight,
            region: request.region.clone(),
            prefix: request.prefix.clone(),
            upstream_presentation: request.upstream_presentation.clone(),
        },
    )
}

fn auth_profile_entry(
    id: &str,
    draft: AuthProfileDraft,
) -> Result<AuthProfileEntry, (StatusCode, Json<serde_json::Value>)> {
    let mut profile = AuthProfileEntry {
        id: id.trim().to_string(),
        mode: draft.mode,
        header: draft.header,
        secret: draft.secret.filter(|value| !value.trim().is_empty()),
        headers: draft.headers,
        disabled: draft.disabled,
        weight: draft.weight,
        region: draft.region,
        prefix: draft.prefix,
        upstream_presentation: draft.upstream_presentation,
        ..Default::default()
    };
    profile.normalize();

    if matches!(profile.mode, AuthMode::ApiKey | AuthMode::BearerToken)
        && !profile.disabled
        && profile.secret.as_deref().is_none_or(str::is_empty)
    {
        return Err(validation_error(
            "secret is required for api-key and bearer-token auth profiles",
        ));
    }
    if profile.mode.is_managed() && profile.secret.is_some() {
        return Err(validation_error(
            "secret must not be set for managed auth profiles",
        ));
    }
    profile
        .validate()
        .map_err(|message| validation_error(&message))?;
    Ok(profile)
}

fn default_managed_header(mode: AuthMode) -> AuthHeaderKind {
    match mode {
        AuthMode::CodexOAuth => AuthHeaderKind::Bearer,
        AuthMode::AnthropicClaudeSubscription => AuthHeaderKind::XApiKey,
        AuthMode::ApiKey | AuthMode::BearerToken => AuthHeaderKind::Auto,
    }
}

fn rebuild_router_from_state(state: &AppState) {
    let config = state.config.load();
    let _ = state.auth_runtime.sync_with_config(&config);
    state
        .router
        .set_oauth_states(state.auth_runtime.oauth_snapshot());
    state.router.update_from_config(&config);
    state
        .catalog
        .update_from_credentials(&state.router.credential_map());
}

fn validation_error(message: &str) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({"error": "validation_failed", "message": message})),
    )
}

fn not_found(message: &str) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(json!({"error": "not_found", "message": message})),
    )
}

fn internal_error(message: impl Into<String>) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": "internal_error", "message": message.into()})),
    )
}

async fn ensure_managed_profile_shape(
    state: &AppState,
    provider: &str,
    profile_id: &str,
    mode: AuthMode,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    let config = state.config.load();
    let Some(entry) = config.providers.iter().find(|entry| entry.name == provider) else {
        return Err(not_found("Provider not found"));
    };
    AuthProfileEntry {
        id: profile_id.to_string(),
        mode,
        header: default_managed_header(mode),
        ..Default::default()
    }
    .validate_for_provider(
        entry.format,
        entry.upstream_kind(),
        entry.base_url.as_deref(),
    )
    .map_err(|message| validation_error(&message))?;
    drop(config);

    match update_config_versioned(state, None, move |config| {
        if let Some(entry) = config
            .providers
            .iter_mut()
            .find(|entry| entry.name == provider)
        {
            migrate_legacy_provider_auth(entry);
            if let Some(profile) = entry
                .auth_profiles
                .iter_mut()
                .find(|profile| profile.id == profile_id)
            {
                profile.mode = mode;
                profile.header = default_managed_header(mode);
                profile.secret = None;
                profile.access_token = None;
                profile.refresh_token = None;
                profile.id_token = None;
                profile.expires_at = None;
                profile.account_id = None;
                profile.email = None;
                profile.last_refresh = None;
                profile.disabled = false;
                return;
            }

            entry.auth_profiles.push(AuthProfileEntry {
                id: profile_id.to_string(),
                mode,
                header: default_managed_header(mode),
                disabled: false,
                ..Default::default()
            });
        }
    })
    .await
    {
        Ok(_) => Ok(()),
        Err(ConfigTxError::Conflict { current_version }) => Err((
            StatusCode::CONFLICT,
            Json(json!({"error": "config_conflict", "current_version": current_version})),
        )),
        Err(ConfigTxError::Validation(message)) => Err(validation_error(&message)),
        Err(ConfigTxError::Internal(message)) => Err(internal_error(message)),
    }
}

/// GET /api/dashboard/auth-profiles
pub async fn list_auth_profiles(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.load();
    let mut profiles = Vec::new();

    for entry in &config.providers {
        for profile in &entry.auth_profiles {
            match hydrate_profile(&state, &entry.name, profile) {
                Ok(hydrated) => {
                    profiles.push(summarize_profile(&entry.name, entry.format, &hydrated))
                }
                Err(response) => return response,
            }
        }
    }

    (StatusCode::OK, Json(json!({ "profiles": profiles })))
}

/// POST /api/dashboard/auth-profiles
pub async fn create_auth_profile(
    State(state): State<AppState>,
    Json(body): Json<CreateAuthProfileRequest>,
) -> impl IntoResponse {
    if body.provider.trim().is_empty() || body.id.trim().is_empty() {
        return validation_error("provider and id are required");
    }

    let profile = match auth_profile_entry_from_create(&body) {
        Ok(profile) => profile,
        Err(response) => return response,
    };

    let config = state.config.load();
    let Some(entry) = config
        .providers
        .iter()
        .find(|entry| entry.name == body.provider)
    else {
        return not_found("Provider not found");
    };
    if let Err(message) = profile.validate_for_provider(
        entry.format,
        entry.upstream_kind(),
        entry.base_url.as_deref(),
    ) {
        return validation_error(&message);
    }
    let duplicate_after_migration = entry.api_key.trim().is_empty()
        && entry.auth_profiles.iter().any(|item| item.id == profile.id);
    let duplicate_legacy_profile = entry.auth_profiles.is_empty()
        && !entry.api_key.trim().is_empty()
        && entry.name == profile.id;
    if duplicate_after_migration || duplicate_legacy_profile {
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "duplicate_auth_profile",
                "message": "auth profile id already exists for provider"
            })),
        );
    }
    drop(config);

    let provider = body.provider.clone();
    let profile_id = profile.id.clone();
    match update_config_versioned(&state, None, move |config| {
        if let Some(entry) = config
            .providers
            .iter_mut()
            .find(|entry| entry.name == provider)
        {
            migrate_legacy_provider_auth(entry);
            entry.auth_profiles.push(profile);
        }
    })
    .await
    {
        Ok(_) => {
            if body.mode.is_managed()
                && let Err(err) = state
                    .auth_runtime
                    .ensure_profile_placeholder(&body.provider, &profile_id)
            {
                return internal_error(err);
            }
            rebuild_router_from_state(&state);
            match current_profile_response(&state, &body.provider, &profile_id) {
                Ok(profile) => (StatusCode::CREATED, Json(json!({ "profile": profile }))),
                Err(response) => response,
            }
        }
        Err(ConfigTxError::Conflict { current_version }) => (
            StatusCode::CONFLICT,
            Json(json!({"error": "config_conflict", "current_version": current_version})),
        ),
        Err(ConfigTxError::Validation(message)) => validation_error(&message),
        Err(ConfigTxError::Internal(message)) => internal_error(message),
    }
}

/// PUT /api/dashboard/auth-profiles/{provider}/{profile}
pub async fn replace_auth_profile(
    State(state): State<AppState>,
    Path((provider, profile_id)): Path<(String, String)>,
    Json(body): Json<ReplaceAuthProfileRequest>,
) -> impl IntoResponse {
    let config = state.config.load();
    let Some(entry) = config.providers.iter().find(|entry| entry.name == provider) else {
        return not_found("Auth profile not found");
    };
    let Some(existing_profile) = entry
        .auth_profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .cloned()
    else {
        return not_found("Auth profile not found");
    };

    let effective_secret = body.secret.clone().or_else(|| {
        (existing_profile.mode == body.mode
            && matches!(body.mode, AuthMode::ApiKey | AuthMode::BearerToken))
        .then(|| existing_profile.secret.clone())
        .flatten()
    });
    let replacement = match auth_profile_entry(
        &profile_id,
        AuthProfileDraft {
            mode: body.mode,
            header: body.header,
            secret: effective_secret,
            headers: body.headers.clone(),
            disabled: body.disabled,
            weight: body.weight,
            region: body.region.clone(),
            prefix: body.prefix.clone(),
            upstream_presentation: body.upstream_presentation.clone(),
        },
    ) {
        Ok(profile) => profile,
        Err(response) => return response,
    };

    if let Err(message) = replacement.validate_for_provider(
        entry.format,
        entry.upstream_kind(),
        entry.base_url.as_deref(),
    ) {
        return validation_error(&message);
    }
    drop(config);

    let profile_was_managed = existing_profile.mode.is_managed();

    let provider_for_update = provider.clone();
    let profile_id_for_update = profile_id.clone();
    match update_config_versioned(&state, None, move |config| {
        if let Some(entry) = config
            .providers
            .iter_mut()
            .find(|entry| entry.name == provider_for_update)
            && let Some(profile) = entry
                .auth_profiles
                .iter_mut()
                .find(|profile| profile.id == profile_id_for_update)
        {
            *profile = replacement;
        }
    })
    .await
    {
        Ok(_) => {
            if body.mode.is_managed() {
                if let Err(err) = state
                    .auth_runtime
                    .ensure_profile_placeholder(&provider, &profile_id)
                {
                    return internal_error(err);
                }
            } else if profile_was_managed
                && let Err(err) = state
                    .auth_runtime
                    .clear_profile_state(&provider, &profile_id)
            {
                return internal_error(err);
            }
            rebuild_router_from_state(&state);
            match current_profile_response(&state, &provider, &profile_id) {
                Ok(profile) => (StatusCode::OK, Json(json!({ "profile": profile }))),
                Err(response) => response,
            }
        }
        Err(ConfigTxError::Conflict { current_version }) => (
            StatusCode::CONFLICT,
            Json(json!({"error": "config_conflict", "current_version": current_version})),
        ),
        Err(ConfigTxError::Validation(message)) => validation_error(&message),
        Err(ConfigTxError::Internal(message)) => internal_error(message),
    }
}

/// DELETE /api/dashboard/auth-profiles/{provider}/{profile}
pub async fn delete_auth_profile(
    State(state): State<AppState>,
    Path((provider, profile_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let existed = explicit_profile(&state.config.load(), &provider, &profile_id).is_some();
    if !existed {
        return not_found("Auth profile not found");
    }

    let provider_for_delete = provider.clone();
    let profile_id_for_delete = profile_id.clone();
    match update_config_versioned(&state, None, move |config| {
        if let Some(entry) = config
            .providers
            .iter_mut()
            .find(|entry| entry.name == provider_for_delete)
        {
            entry
                .auth_profiles
                .retain(|profile| profile.id != profile_id_for_delete);
        }
    })
    .await
    {
        Ok(_) => {
            if let Err(err) = state
                .auth_runtime
                .clear_profile_state(&provider, &profile_id)
            {
                return internal_error(err);
            }
            rebuild_router_from_state(&state);
            (StatusCode::OK, Json(json!({ "deleted": true })))
        }
        Err(ConfigTxError::Conflict { current_version }) => (
            StatusCode::CONFLICT,
            Json(json!({"error": "config_conflict", "current_version": current_version})),
        ),
        Err(ConfigTxError::Validation(message)) => validation_error(&message),
        Err(ConfigTxError::Internal(message)) => internal_error(message),
    }
}

/// POST /api/dashboard/auth-profiles/codex/oauth/start
pub async fn start_codex_oauth(
    State(state): State<AppState>,
    Json(body): Json<StartCodexOauthRequest>,
) -> impl IntoResponse {
    if body.provider.trim().is_empty()
        || body.profile_id.trim().is_empty()
        || body.redirect_uri.trim().is_empty()
    {
        return validation_error("provider, profile_id, and redirect_uri are required");
    }

    if let Err(response) = ensure_managed_profile_shape(
        &state,
        &body.provider,
        &body.profile_id,
        AuthMode::CodexOAuth,
    )
    .await
    {
        return response;
    }

    let state_key = uuid::Uuid::new_v4().to_string();
    let (code_verifier, challenge) = match crate::auth_runtime::AuthRuntimeManager::generate_pkce()
    {
        Ok(value) => value,
        Err(err) => return internal_error(err.to_string()),
    };

    state.oauth_sessions.insert(
        state_key.clone(),
        crate::auth_runtime::PendingCodexOauthSession {
            provider: body.provider.clone(),
            profile_id: body.profile_id.clone(),
            code_verifier,
            redirect_uri: body.redirect_uri.clone(),
            created_at: Utc::now(),
        },
    );

    let auth_url =
        state
            .auth_runtime
            .build_codex_auth_url(&state_key, &challenge, &body.redirect_uri);

    (
        StatusCode::OK,
        Json(json!({
            "state": state_key,
            "auth_url": auth_url,
            "provider": body.provider,
            "profile_id": body.profile_id,
            "expires_in": Duration::minutes(OAUTH_SESSION_TTL_MINUTES).num_seconds(),
        })),
    )
}

/// POST /api/dashboard/auth-profiles/codex/oauth/complete
pub async fn complete_codex_oauth(
    State(state): State<AppState>,
    Json(body): Json<CompleteCodexOauthRequest>,
) -> impl IntoResponse {
    if body.state.trim().is_empty() || body.code.trim().is_empty() {
        return validation_error("state and code are required");
    }

    let Some(session) = state
        .oauth_sessions
        .get(&body.state)
        .map(|entry| entry.clone())
    else {
        return not_found("OAuth session not found");
    };
    if session.created_at + Duration::minutes(OAUTH_SESSION_TTL_MINUTES) < Utc::now() {
        state.oauth_sessions.remove(&body.state);
        return (
            StatusCode::GONE,
            Json(json!({"error": "expired", "message": "OAuth session expired"})),
        );
    }

    let global_proxy = state.config.load().proxy_url.clone();
    let tokens = match state
        .auth_runtime
        .exchange_codex_code(
            &state.http_client_pool,
            global_proxy.as_deref(),
            &body.code,
            &session.redirect_uri,
            &session.code_verifier,
        )
        .await
    {
        Ok(tokens) => tokens,
        Err(message) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "oauth_exchange_failed", "message": message})),
            );
        }
    };

    if let Err(response) = ensure_managed_profile_shape(
        &state,
        &session.provider,
        &session.profile_id,
        AuthMode::CodexOAuth,
    )
    .await
    {
        return response;
    }
    if let Err(err) =
        state
            .auth_runtime
            .store_codex_tokens(&session.provider, &session.profile_id, &tokens)
    {
        return internal_error(err);
    }
    state.oauth_sessions.remove(&body.state);
    rebuild_router_from_state(&state);

    match current_profile_response(&state, &session.provider, &session.profile_id) {
        Ok(profile) => (StatusCode::OK, Json(json!({ "profile": profile }))),
        Err(response) => response,
    }
}

/// POST /api/dashboard/auth-profiles/codex/device/start
pub async fn start_codex_device(
    State(state): State<AppState>,
    Json(body): Json<StartCodexDeviceRequest>,
) -> impl IntoResponse {
    if body.provider.trim().is_empty() || body.profile_id.trim().is_empty() {
        return validation_error("provider and profile_id are required");
    }

    if let Err(response) = ensure_managed_profile_shape(
        &state,
        &body.provider,
        &body.profile_id,
        AuthMode::CodexOAuth,
    )
    .await
    {
        return response;
    }

    let global_proxy = state.config.load().proxy_url.clone();
    let start = match state
        .auth_runtime
        .start_codex_device_flow(&state.http_client_pool, global_proxy.as_deref())
        .await
    {
        Ok(start) => start,
        Err(message) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "device_start_failed", "message": message})),
            );
        }
    };

    let state_key = uuid::Uuid::new_v4().to_string();
    state.device_sessions.insert(
        state_key.clone(),
        crate::auth_runtime::PendingCodexDeviceSession {
            provider: body.provider.clone(),
            profile_id: body.profile_id.clone(),
            device_auth_id: start.device_auth_id.clone(),
            user_code: start.user_code.clone(),
            interval_secs: start.interval_secs,
            created_at: Utc::now(),
        },
    );

    (
        StatusCode::OK,
        Json(json!({
            "state": state_key,
            "provider": body.provider,
            "profile_id": body.profile_id,
            "verification_url": start.verification_url,
            "user_code": start.user_code,
            "interval_secs": start.interval_secs,
            "expires_in": start.expires_in_secs,
        })),
    )
}

/// POST /api/dashboard/auth-profiles/codex/device/poll
pub async fn poll_codex_device(
    State(state): State<AppState>,
    Json(body): Json<PollCodexDeviceRequest>,
) -> impl IntoResponse {
    if body.state.trim().is_empty() {
        return validation_error("state is required");
    }

    let Some(session) = state
        .device_sessions
        .get(&body.state)
        .map(|entry| entry.clone())
    else {
        return not_found("Device session not found");
    };
    if session.created_at + Duration::minutes(DEVICE_SESSION_TTL_MINUTES) < Utc::now() {
        state.device_sessions.remove(&body.state);
        return (
            StatusCode::GONE,
            Json(json!({"error": "expired", "message": "Device session expired"})),
        );
    }

    let global_proxy = state.config.load().proxy_url.clone();
    let result = match state
        .auth_runtime
        .poll_codex_device_flow(&state.http_client_pool, global_proxy.as_deref(), &session)
        .await
    {
        Ok(result) => result,
        Err(message) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "device_poll_failed", "message": message})),
            );
        }
    };

    match result {
        crate::auth_runtime::CodexDevicePollResult::Pending => (
            StatusCode::OK,
            Json(json!({
                "status": "pending",
                "interval_secs": session.interval_secs,
            })),
        ),
        crate::auth_runtime::CodexDevicePollResult::Complete(tokens) => {
            if let Err(response) = ensure_managed_profile_shape(
                &state,
                &session.provider,
                &session.profile_id,
                AuthMode::CodexOAuth,
            )
            .await
            {
                return response;
            }
            if let Err(err) = state.auth_runtime.store_codex_tokens(
                &session.provider,
                &session.profile_id,
                &tokens,
            ) {
                return internal_error(err);
            }
            state.device_sessions.remove(&body.state);
            rebuild_router_from_state(&state);

            match current_profile_response(&state, &session.provider, &session.profile_id) {
                Ok(profile) => (
                    StatusCode::OK,
                    Json(json!({ "status": "completed", "profile": profile })),
                ),
                Err(response) => response,
            }
        }
    }
}

/// POST /api/dashboard/auth-profiles/{provider}/{profile}/import-local
pub async fn import_local_auth_profile(
    State(state): State<AppState>,
    Path((provider, profile_id)): Path<(String, String)>,
) -> impl IntoResponse {
    if let Err(response) =
        ensure_managed_profile_shape(&state, &provider, &profile_id, AuthMode::CodexOAuth).await
    {
        return response;
    }

    let tokens = match state.auth_runtime.load_codex_cli_tokens(None) {
        Ok(tokens) => tokens,
        Err(message) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "local_import_failed", "message": message})),
            );
        }
    };
    if let Err(err) = state
        .auth_runtime
        .store_codex_tokens(&provider, &profile_id, &tokens)
    {
        return internal_error(err);
    }
    rebuild_router_from_state(&state);

    match current_profile_response(&state, &provider, &profile_id) {
        Ok(profile) => (StatusCode::OK, Json(json!({ "profile": profile }))),
        Err(response) => response,
    }
}

/// POST /api/dashboard/auth-profiles/{provider}/{profile}/connect
pub async fn connect_auth_profile(
    State(state): State<AppState>,
    Path((provider, profile_id)): Path<(String, String)>,
    Json(body): Json<ConnectAuthProfileRequest>,
) -> impl IntoResponse {
    if body.secret.trim().is_empty() {
        return validation_error("secret is required");
    }

    let config = state.config.load();
    let Some((entry, profile)) = explicit_profile(&config, &provider, &profile_id) else {
        return not_found("Auth profile not found");
    };
    if profile.mode != AuthMode::AnthropicClaudeSubscription {
        return validation_error(
            "connect is only supported for anthropic-claude-subscription profiles",
        );
    }
    if let Err(message) = profile.validate_for_provider(
        entry.format,
        entry.upstream_kind(),
        entry.base_url.as_deref(),
    ) {
        return validation_error(&message);
    }
    if let Err(message) = validate_anthropic_subscription_token(body.secret.trim()) {
        return validation_error(&message);
    }
    drop(config);

    if let Err(response) = ensure_managed_profile_shape(
        &state,
        &provider,
        &profile_id,
        AuthMode::AnthropicClaudeSubscription,
    )
    .await
    {
        return response;
    }

    if let Err(err) = state.auth_runtime.store_anthropic_subscription_token(
        &provider,
        &profile_id,
        body.secret.trim(),
    ) {
        return internal_error(err);
    }
    rebuild_router_from_state(&state);

    match current_profile_response(&state, &provider, &profile_id) {
        Ok(profile) => (StatusCode::OK, Json(json!({ "profile": profile }))),
        Err(response) => response,
    }
}

/// POST /api/dashboard/auth-profiles/{provider}/{profile}/refresh
pub async fn refresh_auth_profile(
    State(state): State<AppState>,
    Path((provider, profile_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let config = state.config.load();
    let Some((_, profile)) = explicit_profile(&config, &provider, &profile_id) else {
        return not_found("Auth profile not found");
    };
    if !profile.mode.supports_refresh() {
        return validation_error("refresh is only supported for refreshable managed auth profiles");
    }

    let oauth_state = match state.auth_runtime.state_for_profile(&provider, &profile_id) {
        Ok(Some(runtime_state)) => runtime_state,
        Ok(None) => match OAuthTokenState::from_profile(profile) {
            Some(runtime_state) => runtime_state,
            None => {
                return validation_error(
                    "auth profile is disconnected; reconnect it before refresh",
                );
            }
        },
        Err(message) => return internal_error(message),
    };
    drop(config);

    let global_proxy = state.config.load().proxy_url.clone();
    let tokens = match state
        .auth_runtime
        .refresh_codex_tokens(
            &state.http_client_pool,
            global_proxy.as_deref(),
            Arc::new(RwLock::new(oauth_state)),
        )
        .await
    {
        Ok(tokens) => tokens,
        Err(err) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "oauth_refresh_failed", "message": err.to_string()})),
            );
        }
    };

    if let Err(err) = state
        .auth_runtime
        .store_codex_tokens(&provider, &profile_id, &tokens)
    {
        return internal_error(err);
    }
    rebuild_router_from_state(&state);

    match current_profile_response(&state, &provider, &profile_id) {
        Ok(profile) => (StatusCode::OK, Json(json!({ "profile": profile }))),
        Err(response) => response,
    }
}
