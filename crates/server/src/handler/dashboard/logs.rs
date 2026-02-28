use crate::AppState;
use ai_proxy_core::request_log::LogQuery;
use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde_json::json;

/// GET /api/dashboard/logs — query request logs with filters.
pub async fn query_logs(
    State(state): State<AppState>,
    Query(query): Query<LogQuery>,
) -> impl IntoResponse {
    let page = state.request_logs.query(&query);
    (StatusCode::OK, Json(json!(page)))
}

/// GET /api/dashboard/logs/stats — request log statistics.
pub async fn log_stats(State(state): State<AppState>) -> impl IntoResponse {
    let stats = state.request_logs.stats();
    (StatusCode::OK, Json(stats))
}
