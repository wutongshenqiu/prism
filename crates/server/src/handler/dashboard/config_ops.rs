use crate::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde_json::json;

/// POST /api/dashboard/config/validate — dry-run config validation.
/// Accepts either `{"yaml": "..."}` (YAML string) or a raw JSON config object.
pub async fn validate_config(
    State(_state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let result = if let Some(yaml_str) = body.get("yaml").and_then(|v| v.as_str()) {
        prism_core::config::Config::load_from_str(yaml_str).map(|_| ())
    } else {
        serde_json::from_value::<prism_core::config::Config>(body)
            .map(|_| ())
            .map_err(|e| anyhow::anyhow!("{e}"))
    };
    match result {
        Ok(()) => (StatusCode::OK, Json(json!({"valid": true, "errors": []}))),
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "valid": false,
                "errors": [e.to_string()],
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

    match prism_core::config::Config::load(&config_path) {
        Ok(new_cfg) => {
            state.router.update_from_config(&new_cfg);
            state.rate_limiter.update_config(&new_cfg.rate_limit);
            state.cost_calculator.update_prices(&new_cfg.model_prices);
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

/// GET /api/dashboard/config/raw — get raw YAML config file contents.
pub async fn get_raw_config(State(state): State<AppState>) -> impl IntoResponse {
    let config_path = match state.config_path.lock() {
        Ok(path) => path.clone(),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "lock_failed", "message": e.to_string()})),
            );
        }
    };

    match std::fs::read_to_string(&config_path) {
        Ok(content) => (
            StatusCode::OK,
            Json(json!({"content": content, "path": config_path})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "read_failed", "message": e.to_string()})),
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
        "auth_keys_count": config.auth_keys.len(),
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
            "log_store_capacity": config.log_store.capacity,
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
