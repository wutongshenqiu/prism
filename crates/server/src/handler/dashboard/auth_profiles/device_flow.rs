use super::{
    current_profile_response, ensure_managed_profile_shape, managed_auth_proxy_url, not_found,
    rebuild_router_from_state, validation_error,
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

const DEVICE_SESSION_TTL_MINUTES: i64 = 15;

#[derive(Debug, Deserialize)]
pub struct StartCodexDeviceRequest {
    pub provider: String,
    pub profile_id: String,
}

#[derive(Debug, Deserialize)]
pub struct PollCodexDeviceRequest {
    pub state: String,
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

    let auth_proxy = managed_auth_proxy_url(&state);
    let start = match state
        .auth_runtime
        .start_codex_device_flow(&state.http_client_pool, auth_proxy.as_deref())
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

    let auth_proxy = managed_auth_proxy_url(&state);
    let result = match state
        .auth_runtime
        .poll_codex_device_flow(&state.http_client_pool, auth_proxy.as_deref(), &session)
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
            Json(json!({"status": "pending", "interval_secs": session.interval_secs})),
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
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "store_failed", "message": err})),
                );
            }
            state.device_sessions.remove(&body.state);
            rebuild_router_from_state(&state);
            match current_profile_response(&state, &session.provider, &session.profile_id) {
                Ok(profile) => (
                    StatusCode::OK,
                    Json(json!({"status": "completed", "profile": profile})),
                ),
                Err(response) => response,
            }
        }
    }
}
