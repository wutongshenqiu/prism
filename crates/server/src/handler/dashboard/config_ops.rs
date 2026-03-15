use crate::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde_json::json;

fn config_tx_error_response(
    error: super::config_tx::ConfigTxError,
) -> (StatusCode, Json<serde_json::Value>) {
    match error {
        super::config_tx::ConfigTxError::Conflict { current_version } => (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "config_conflict",
                "message": "Configuration has been modified by another session. Refresh and retry.",
                "current_version": current_version,
            })),
        ),
        super::config_tx::ConfigTxError::Validation(message) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": "validation_failed", "message": message})),
        ),
        super::config_tx::ConfigTxError::Internal(message) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "write_failed", "message": message})),
        ),
    }
}

/// POST /api/dashboard/config/validate — dry-run config validation.
/// Accepts either `{"yaml": "..."}` (YAML string) or a raw JSON config object.
///
/// Performs two validation phases:
/// 1. Structural parsing (YAML/JSON → Config)
/// 2. Full resolution including secrets (env://, file://)
pub async fn validate_config(
    State(_state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let yaml_str;
    let parse_result = if let Some(s) = body.get("yaml").and_then(|v| v.as_str()) {
        yaml_str = s.to_string();
        // Phase 1: raw parse — catches structural/schema errors
        prism_core::config::Config::from_yaml_raw(&yaml_str)
    } else {
        match serde_json::from_value::<prism_core::config::Config>(body) {
            Ok(_cfg) => {
                return (StatusCode::OK, Json(json!({"valid": true, "errors": []})));
            }
            Err(e) => {
                return (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({"valid": false, "errors": [e.to_string()]})),
                );
            }
        }
    };

    let mut errors = Vec::new();

    match parse_result {
        Ok(raw_cfg) => {
            // Phase 1 passed. Phase 2: full resolution (secrets, validation)
            // Check for auth fields that reference unresolvable secrets
            for (i, p) in raw_cfg.providers.iter().enumerate() {
                if (p.api_key.starts_with("env://") || p.api_key.starts_with("file://"))
                    && let Err(e) = prism_core::secret::resolve(&p.api_key)
                {
                    errors.push(format!(
                        "providers[{}] '{}': api_key secret resolution failed: {}",
                        i, p.name, e
                    ));
                }
            }
            for (i, ak) in raw_cfg.auth_keys.iter().enumerate() {
                if (ak.key.starts_with("env://") || ak.key.starts_with("file://"))
                    && let Err(e) = prism_core::secret::resolve(&ak.key)
                {
                    errors.push(format!(
                        "auth-keys[{}]: key secret resolution failed: {}",
                        i, e
                    ));
                }
            }

            if !errors.is_empty() {
                return (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({"valid": false, "errors": errors})),
                );
            }

            // Full validation with resolution
            match prism_core::config::Config::load_from_str(&raw_cfg.to_yaml().unwrap_or_default())
            {
                Ok(_) => (StatusCode::OK, Json(json!({"valid": true, "errors": []}))),
                Err(e) => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({"valid": false, "errors": [e.to_string()]})),
                ),
            }
        }
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"valid": false, "errors": [e.to_string()]})),
        ),
    }
}

/// POST /api/dashboard/config/reload — trigger hot-reload.
pub async fn reload_config(State(state): State<AppState>) -> impl IntoResponse {
    let config_path = state
        .config_path
        .lock()
        .map(|path| path.clone())
        .unwrap_or_default();

    match super::config_tx::reload_config_from_disk(&state).await {
        Ok(()) => {
            tracing::info!(path = %config_path, "Configuration reloaded via dashboard API");
            (
                StatusCode::OK,
                Json(json!({"message": "Configuration reloaded successfully"})),
            )
        }
        Err(super::config_tx::ConfigTxError::Validation(message)) => {
            tracing::error!(path = %config_path, error = %message, "Configuration reload failed");
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({"error": "reload_failed", "message": message})),
            )
        }
        Err(super::config_tx::ConfigTxError::Internal(message)) => {
            tracing::error!(path = %config_path, error = %message, "Configuration reload failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "reload_failed", "message": message})),
            )
        }
        Err(super::config_tx::ConfigTxError::Conflict { .. }) => {
            tracing::error!(
                path = %config_path,
                "Configuration reload hit an unexpected version conflict"
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(
                    json!({"error": "reload_failed", "message": "Unexpected config version conflict during reload"}),
                ),
            )
        }
    }
}

/// PUT /api/dashboard/config/apply — validate, persist, and reload config.
/// Accepts `{"yaml": "...", "config_version": "..."}`.
/// If `config_version` is provided and doesn't match the current file, returns 409 Conflict.
pub async fn apply_config(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let yaml_str = match body.get("yaml").and_then(|v| v.as_str()) {
        Some(s) => s.to_string(),
        None => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({"error": "validation_failed", "message": "Missing 'yaml' field"})),
            );
        }
    };

    let expected_version = body
        .get("config_version")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let config_path = state
        .config_path
        .lock()
        .map(|path| path.clone())
        .unwrap_or_default();

    match super::config_tx::apply_yaml_versioned(&state, &yaml_str, expected_version.as_deref())
        .await
    {
        Ok(new_version) => {
            tracing::info!(path = %config_path, "Configuration applied via dashboard API");
            (
                StatusCode::OK,
                Json(json!({
                    "message": "Configuration applied successfully",
                    "config_version": new_version,
                })),
            )
        }
        Err(error) => config_tx_error_response(error),
    }
}

/// GET /api/dashboard/config/raw — get raw YAML config file contents with version.
pub async fn get_raw_config(State(state): State<AppState>) -> impl IntoResponse {
    match super::config_tx::read_config_versioned(&state) {
        Ok((content, version)) => {
            let config_path = state
                .config_path
                .lock()
                .map(|p| p.clone())
                .unwrap_or_default();
            (
                StatusCode::OK,
                Json(json!({"content": content, "path": config_path, "config_version": version})),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "read_failed", "message": e})),
        ),
    }
}

/// GET /api/dashboard/config/current — get full sanitized config with version.
/// Returns a truthful view of all configuration sections. Secrets are masked.
pub async fn get_config(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.load();
    let version = super::config_tx::read_config_versioned(&state)
        .map(|(_, v)| v)
        .unwrap_or_default();

    // Build providers summary (mask API keys)
    let providers_summary: Vec<serde_json::Value> = config
        .providers
        .iter()
        .map(|p| {
            json!({
                "name": p.name,
                "format": p.format.as_str(),
                "disabled": p.disabled,
                "models_count": p.models.len(),
                "region": p.region,
                "wire_api": p.wire_api,
            })
        })
        .collect();

    let sanitized = json!({
        "listen": {
            "host": config.host,
            "port": config.port,
            "tls_enabled": config.tls.enable,
            "body_limit_mb": config.body_limit_mb,
        },
        "providers": {
            "total": config.providers.len(),
            "items": providers_summary,
        },
        "routing": config.routing,
        "auth_keys": {
            "total": config.auth_keys.len(),
        },
        "dashboard": {
            "enabled": config.dashboard.enabled,
            "username": config.dashboard.username,
            "jwt_ttl_secs": config.dashboard.jwt_ttl_secs,
        },
        "rate_limit": config.rate_limit,
        "cache": {
            "enabled": config.cache.enabled,
            "max_entries": config.cache.max_entries,
            "ttl_secs": config.cache.ttl_secs,
        },
        "cost": {
            "custom_prices_count": config.model_prices.len(),
        },
        "retry": config.retry,
        "streaming": config.streaming,
        "timeouts": {
            "connect_timeout": config.connect_timeout,
            "request_timeout": config.request_timeout,
        },
        "log_store": {
            "capacity": config.log_store.capacity,
        },
        "config_version": version,
    });
    (StatusCode::OK, Json(sanitized))
}
