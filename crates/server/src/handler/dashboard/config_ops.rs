use crate::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde_json::json;

/// POST /api/dashboard/config/validate — dry-run config validation.
pub async fn validate_config(
    State(_state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    // Attempt to deserialize as Config
    let result: Result<ai_proxy_core::config::Config, _> = serde_json::from_value(body);
    match result {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({"valid": true, "message": "Configuration is valid"})),
        ),
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "valid": false,
                "error": "validation_failed",
                "message": e.to_string(),
            })),
        ),
    }
}

/// POST /api/dashboard/config/reload — trigger hot-reload.
pub async fn reload_config(State(state): State<AppState>) -> impl IntoResponse {
    let config_path = match state.config_path.lock() {
        Ok(path) => path.clone(),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "lock_failed", "message": e.to_string()})),
            );
        }
    };

    match ai_proxy_core::config::Config::load(&config_path) {
        Ok(new_cfg) => {
            state.credential_router.update_from_config(&new_cfg);
            state.config.store(std::sync::Arc::new(new_cfg));
            (
                StatusCode::OK,
                Json(json!({"message": "Configuration reloaded successfully"})),
            )
        }
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": "reload_failed", "message": e.to_string()})),
        ),
    }
}

/// GET /api/dashboard/config/current — get full sanitized config.
pub async fn get_config(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.load();
    let sanitized = json!({
        "host": config.host,
        "port": config.port,
        "tls": { "enable": config.tls.enable },
        "api_keys_count": config.api_keys.len(),
        "routing": config.routing,
        "retry": config.retry,
        "body_limit_mb": config.body_limit_mb,
        "streaming": config.streaming,
        "connect_timeout": config.connect_timeout,
        "request_timeout": config.request_timeout,
        "dashboard": {
            "enabled": config.dashboard.enabled,
            "username": config.dashboard.username,
            "jwt_ttl_secs": config.dashboard.jwt_ttl_secs,
            "request_log_capacity": config.dashboard.request_log_capacity,
        },
        "providers": {
            "claude": config.claude_api_key.len(),
            "openai": config.openai_api_key.len(),
            "gemini": config.gemini_api_key.len(),
            "openai_compat": config.openai_compatibility.len(),
        },
    });
    (StatusCode::OK, Json(sanitized))
}
