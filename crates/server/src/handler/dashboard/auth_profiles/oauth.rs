use super::{
    current_profile_response, ensure_managed_profile_shape, internal_error, managed_auth_proxy_url,
    not_found, rebuild_router_from_state, validation_error,
};
use crate::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use chrono::{Duration, Utc};
use prism_core::auth_profile::AuthMode;
use serde::Deserialize;
use serde_json::json;

const OAUTH_SESSION_TTL_MINUTES: i64 = 10;

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

    let auth_proxy = managed_auth_proxy_url(&state);
    let tokens = match state
        .auth_runtime
        .exchange_codex_code(
            &state.http_client_pool,
            auth_proxy.as_deref(),
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
