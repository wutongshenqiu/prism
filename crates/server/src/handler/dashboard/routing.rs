use crate::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct UpdateRoutingRequest {
    pub strategy: String,
}

/// GET /api/dashboard/routing
pub async fn get_routing(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.load();
    (
        StatusCode::OK,
        Json(json!({
            "strategy": config.routing.strategy,
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
    let strategy = match body.strategy.as_str() {
        "round-robin" | "RoundRobin" => ai_proxy_core::config::RoutingStrategy::RoundRobin,
        "fill-first" | "FillFirst" => ai_proxy_core::config::RoutingStrategy::FillFirst,
        _ => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(
                    json!({"error": "validation_failed", "message": "Invalid strategy. Must be 'round-robin' or 'fill-first'"}),
                ),
            );
        }
    };

    match super::providers::update_config_file_public(&state, move |config| {
        config.routing.strategy = strategy;
    })
    .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({"message": "Routing strategy updated successfully"})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "write_failed", "message": e})),
        ),
    }
}
