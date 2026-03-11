use crate::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use prism_core::config::RoutingStrategy;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct UpdateRoutingRequest {
    pub strategy: Option<RoutingStrategy>,
    pub request_retry: Option<u32>,
    pub max_retry_interval: Option<u64>,
    pub fallback_enabled: Option<bool>,
    pub model_strategies: Option<std::collections::HashMap<String, RoutingStrategy>>,
    pub model_fallbacks: Option<std::collections::HashMap<String, Vec<String>>>,
}

/// GET /api/dashboard/routing
pub async fn get_routing(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.load();
    (
        StatusCode::OK,
        Json(json!({
            "strategy": config.routing.strategy,
            "fallback_enabled": config.routing.fallback_enabled,
            "request_retry": config.request_retry,
            "max_retry_interval": config.max_retry_interval,
            "model_strategies": config.routing.model_strategies,
            "model_fallbacks": config.routing.model_fallbacks,
        })),
    )
}

/// PATCH /api/dashboard/routing
pub async fn update_routing(
    State(state): State<AppState>,
    Json(body): Json<UpdateRoutingRequest>,
) -> impl IntoResponse {
    match super::providers::update_config_file_public(&state, move |config| {
        if let Some(s) = body.strategy {
            config.routing.strategy = s;
        }
        if let Some(fb) = body.fallback_enabled {
            config.routing.fallback_enabled = fb;
        }
        if let Some(rr) = body.request_retry {
            config.request_retry = rr;
        }
        if let Some(mri) = body.max_retry_interval {
            config.max_retry_interval = mri;
        }
        if let Some(ms) = body.model_strategies {
            config.routing.model_strategies = ms;
        }
        if let Some(mf) = body.model_fallbacks {
            config.routing.model_fallbacks = mf;
        }
    })
    .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({"message": "Routing configuration updated successfully"})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "write_failed", "message": e})),
        ),
    }
}
