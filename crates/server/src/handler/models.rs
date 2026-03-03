use crate::AppState;
use axum::{Json, extract::State, response::IntoResponse};
use prism_core::error::ProxyError;

pub async fn list_models(State(state): State<AppState>) -> Result<impl IntoResponse, ProxyError> {
    let models = state.router.all_models();
    let created = chrono::Utc::now().timestamp();

    let data: Vec<serde_json::Value> = models
        .into_iter()
        .map(|m| {
            serde_json::json!({
                "id": m.id,
                "object": "model",
                "created": created,
                "owned_by": m.owned_by,
            })
        })
        .collect();

    let response = serde_json::json!({
        "object": "list",
        "data": data,
    });

    Ok(Json(response))
}
