mod codex;
mod common;
mod health;
mod models;

use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;
use serde_json::json;

pub use health::{cached_probe_result, health_check};
pub use models::fetch_models;

#[derive(Debug, Deserialize)]
pub struct FetchModelsRequest {
    pub format: String,
    #[serde(default)]
    pub upstream: Option<String>,
    pub api_key: String,
    #[serde(default)]
    pub base_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PresentationPreviewRequest {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub user_agent: Option<String>,
    #[serde(default)]
    pub sample_body: Option<serde_json::Value>,
}

pub async fn presentation_preview(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(body): Json<PresentationPreviewRequest>,
) -> impl IntoResponse {
    let config = state.config.load();

    let entry = match config.providers.iter().find(|entry| entry.name == name) {
        Some(entry) => entry,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "not_found", "message": "Provider not found"})),
            );
        }
    };

    let mut payload = body
        .sample_body
        .unwrap_or_else(|| json!({"messages": [{"role": "user", "content": "hello"}]}));

    let ctx = prism_core::presentation::PresentationContext {
        target_format: entry.format,
        model: body.model.as_deref().unwrap_or("unknown"),
        user_agent: body.user_agent.as_deref(),
        api_key: &entry.api_key,
    };

    let result = prism_core::presentation::apply(&entry.upstream_presentation, &ctx, &mut payload);

    (
        StatusCode::OK,
        Json(json!({
            "profile": result.trace.profile,
            "activated": result.trace.activated,
            "effective_headers": result.headers,
            "body_mutations": result.trace.body_mutations,
            "protected_headers_blocked": result.trace.protected_blocked,
            "effective_body": payload,
        })),
    )
}
