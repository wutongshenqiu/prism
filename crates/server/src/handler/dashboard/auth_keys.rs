use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use prism_core::auth_key::{AuthKeyEntry, AuthKeyStore};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct CreateAuthKeyRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub tenant_id: Option<String>,
    #[serde(default)]
    pub allowed_models: Vec<String>,
    #[serde(default)]
    pub allowed_credentials: Vec<String>,
    #[serde(default)]
    pub rate_limit: Option<prism_core::auth_key::KeyRateLimitConfig>,
    #[serde(default)]
    pub budget: Option<prism_core::auth_key::BudgetConfig>,
    #[serde(default)]
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub metadata: std::collections::HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAuthKeyRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub tenant_id: Option<Option<String>>,
    #[serde(default)]
    pub allowed_models: Option<Vec<String>>,
    #[serde(default)]
    pub allowed_credentials: Option<Vec<String>>,
    #[serde(default)]
    pub rate_limit: Option<Option<prism_core::auth_key::KeyRateLimitConfig>>,
    #[serde(default)]
    pub budget: Option<Option<prism_core::auth_key::BudgetConfig>>,
    #[serde(default)]
    pub expires_at: Option<Option<chrono::DateTime<chrono::Utc>>>,
    #[serde(default)]
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

/// GET /api/dashboard/auth-keys
pub async fn list_auth_keys(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.load();
    let keys: Vec<serde_json::Value> = config
        .auth_keys
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            json!({
                "id": i,
                "key_masked": AuthKeyStore::mask_key(&entry.key),
                "name": entry.name,
                "tenant_id": entry.tenant_id,
                "allowed_models": entry.allowed_models,
                "allowed_credentials": entry.allowed_credentials,
                "rate_limit": entry.rate_limit,
                "budget": entry.budget,
                "expires_at": entry.expires_at,
                "metadata": entry.metadata,
            })
        })
        .collect();
    (StatusCode::OK, Json(json!({ "auth_keys": keys })))
}

/// POST /api/dashboard/auth-keys
pub async fn create_auth_key(
    State(state): State<AppState>,
    Json(body): Json<CreateAuthKeyRequest>,
) -> impl IntoResponse {
    let key = format!(
        "sk-proxy-{}",
        uuid::Uuid::new_v4().to_string().replace('-', "")
    );

    let full_key = key.clone();
    let entry = AuthKeyEntry {
        key,
        name: body.name,
        tenant_id: body.tenant_id,
        allowed_models: body.allowed_models,
        allowed_credentials: body.allowed_credentials,
        rate_limit: body.rate_limit,
        budget: body.budget,
        expires_at: body.expires_at,
        metadata: body.metadata,
    };

    let key_name = entry.name.clone();
    match super::providers::update_config_file_public(&state, move |config| {
        config.auth_keys.push(entry);
        config.auth_key_store = AuthKeyStore::new(config.auth_keys.clone());
    })
    .await
    {
        Ok(_) => {
            tracing::info!(name = ?key_name, "Auth key created via dashboard");
            (
                StatusCode::CREATED,
                Json(json!({
                    "key": full_key,
                    "message": "API key created. Save this key - it will not be shown again.",
                })),
            )
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to create auth key");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "write_failed", "message": e})),
            )
        }
    }
}

/// PATCH /api/dashboard/auth-keys/:id
pub async fn update_auth_key(
    State(state): State<AppState>,
    Path(id): Path<usize>,
    Json(body): Json<UpdateAuthKeyRequest>,
) -> impl IntoResponse {
    match super::providers::update_config_file_public(&state, move |config| {
        if id < config.auth_keys.len() {
            let entry = &mut config.auth_keys[id];
            if let Some(name) = body.name {
                entry.name = Some(name);
            }
            if let Some(tenant_id) = body.tenant_id {
                entry.tenant_id = tenant_id;
            }
            if let Some(allowed_models) = body.allowed_models {
                entry.allowed_models = allowed_models;
            }
            if let Some(allowed_credentials) = body.allowed_credentials {
                entry.allowed_credentials = allowed_credentials;
            }
            if let Some(rate_limit) = body.rate_limit {
                entry.rate_limit = rate_limit;
            }
            if let Some(budget) = body.budget {
                entry.budget = budget;
            }
            if let Some(expires_at) = body.expires_at {
                entry.expires_at = expires_at;
            }
            if let Some(metadata) = body.metadata {
                entry.metadata = metadata;
            }
            config.auth_key_store = AuthKeyStore::new(config.auth_keys.clone());
        }
    })
    .await
    {
        Ok(_) => {
            tracing::info!(key_id = id, "Auth key updated via dashboard");
            (
                StatusCode::OK,
                Json(json!({"message": "Auth key updated successfully"})),
            )
        }
        Err(e) => {
            tracing::error!(key_id = id, error = %e, "Failed to update auth key");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "write_failed", "message": e})),
            )
        }
    }
}

/// POST /api/dashboard/auth-keys/:id/reveal
pub async fn reveal_auth_key(
    State(state): State<AppState>,
    Path(id): Path<usize>,
) -> impl IntoResponse {
    let config = state.config.load();
    if let Some(entry) = config.auth_keys.get(id) {
        tracing::info!(key_id = id, name = ?entry.name, "Auth key revealed via dashboard");
        (StatusCode::OK, Json(json!({ "key": entry.key })))
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "not_found", "message": "Auth key not found"})),
        )
    }
}

/// DELETE /api/dashboard/auth-keys/:id
pub async fn delete_auth_key(
    State(state): State<AppState>,
    Path(id): Path<usize>,
) -> impl IntoResponse {
    match super::providers::update_config_file_public(&state, move |config| {
        if id < config.auth_keys.len() {
            config.auth_keys.remove(id);
            config.auth_key_store = AuthKeyStore::new(config.auth_keys.clone());
        }
    })
    .await
    {
        Ok(_) => {
            tracing::info!(key_id = id, "Auth key deleted via dashboard");
            (
                StatusCode::OK,
                Json(json!({"message": "API key deleted successfully"})),
            )
        }
        Err(e) => {
            tracing::error!(key_id = id, error = %e, "Failed to delete auth key");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "write_failed", "message": e})),
            )
        }
    }
}
