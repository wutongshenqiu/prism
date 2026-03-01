use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;
use serde_json::json;

fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        return "****".to_string();
    }
    format!("{}****{}", &key[..4], &key[key.len() - 4..])
}

#[derive(Debug, Deserialize)]
pub struct CreateAuthKeyRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub expires_in_days: Option<u32>,
}

/// GET /api/dashboard/auth-keys
pub async fn list_auth_keys(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.load();
    let keys: Vec<serde_json::Value> = config
        .api_keys
        .iter()
        .enumerate()
        .map(|(i, k)| {
            json!({
                "id": i,
                "name": format!("Key {}", i + 1),
                "key_masked": mask_key(k),
                "created_at": null,
                "last_used_at": null,
                "expires_at": null,
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
    // Generate a secure random key with optional name prefix
    let name = body.name.clone().unwrap_or_default();
    let key = format!(
        "sk-proxy-{}",
        uuid::Uuid::new_v4().to_string().replace('-', "")
    );

    let expires_at = body.expires_in_days.map(|days| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let expires = now + (days as u64) * 86400;
        // Format as ISO 8601
        let dt = chrono::DateTime::from_timestamp(expires as i64, 0);
        dt.map(|d| d.to_rfc3339()).unwrap_or_default()
    });

    let full_key = key.clone();
    match super::providers::update_config_file_public(&state, move |config| {
        config.api_keys.push(key);
        config.api_keys_set = config.api_keys.iter().cloned().collect();
    })
    .await
    {
        Ok(()) => (
            StatusCode::CREATED,
            Json(json!({
                "key": full_key,
                "name": name,
                "expires_at": expires_at,
                "message": "API key created. Save this key - it will not be shown again.",
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "write_failed", "message": e})),
        ),
    }
}

/// DELETE /api/dashboard/auth-keys/:id
pub async fn delete_auth_key(
    State(state): State<AppState>,
    Path(id): Path<usize>,
) -> impl IntoResponse {
    match super::providers::update_config_file_public(&state, move |config| {
        if id < config.api_keys.len() {
            config.api_keys.remove(id);
            config.api_keys_set = config.api_keys.iter().cloned().collect();
        }
    })
    .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({"message": "API key deleted successfully"})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "write_failed", "message": e})),
        ),
    }
}
