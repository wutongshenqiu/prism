use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde_json::json;

fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        return "****".to_string();
    }
    format!("{}****{}", &key[..4], &key[key.len() - 4..])
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
                "key_masked": mask_key(k),
            })
        })
        .collect();
    (StatusCode::OK, Json(json!({ "auth_keys": keys })))
}

/// POST /api/dashboard/auth-keys
pub async fn create_auth_key(State(state): State<AppState>) -> impl IntoResponse {
    // Generate a secure random key
    let key = format!(
        "sk-proxy-{}",
        uuid::Uuid::new_v4().to_string().replace('-', "")
    );

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
