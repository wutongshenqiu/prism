use crate::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct UpdateRoutingRequest {
    pub strategy: Option<String>,
    pub request_retry: Option<u32>,
    pub max_retry_interval: Option<u64>,
    pub fallback_enabled: Option<bool>,
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
        })),
    )
}

/// PATCH /api/dashboard/routing
pub async fn update_routing(
    State(state): State<AppState>,
    Json(body): Json<UpdateRoutingRequest>,
) -> impl IntoResponse {
    let strategy = if let Some(ref s) = body.strategy {
        match s.as_str() {
            "round-robin" | "RoundRobin" => {
                Some(ai_proxy_core::config::RoutingStrategy::RoundRobin)
            }
            "fill-first" | "FillFirst" => {
                Some(ai_proxy_core::config::RoutingStrategy::FillFirst)
            }
            _ => {
                return (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(
                        json!({"error": "validation_failed", "message": "Invalid strategy. Must be 'round-robin' or 'fill-first'"}),
                    ),
                );
            }
        }
    } else {
        None
    };

    let fallback_enabled = body.fallback_enabled;
    let request_retry = body.request_retry;
    let max_retry_interval = body.max_retry_interval;

    match super::providers::update_config_file_public(&state, move |config| {
        if let Some(s) = strategy {
            config.routing.strategy = s;
        }
        if let Some(fb) = fallback_enabled {
            config.routing.fallback_enabled = fb;
        }
        if let Some(rr) = request_retry {
            config.request_retry = rr;
        }
        if let Some(mri) = max_retry_interval {
            config.max_retry_interval = mri;
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
