use crate::AppState;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use prism_core::request_log::{LogQuery, StatsQuery};

/// GET /api/dashboard/logs — query request logs with filters.
pub async fn query_logs(
    State(state): State<AppState>,
    Query(query): Query<LogQuery>,
) -> impl IntoResponse {
    let page = state.log_store.query(&query).await;
    (StatusCode::OK, Json(page))
}

/// GET /api/dashboard/logs/:id — get a single log entry by request ID.
pub async fn get_log(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    match state.log_store.get(&id).await {
        Some(record) => Json(record).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// GET /api/dashboard/logs/stats — request log statistics.
pub async fn log_stats(
    State(state): State<AppState>,
    Query(query): Query<StatsQuery>,
) -> impl IntoResponse {
    let stats = state.log_store.stats(&query).await;
    (StatusCode::OK, Json(stats))
}

/// GET /api/dashboard/logs/filters — available filter options.
pub async fn filter_options(State(state): State<AppState>) -> impl IntoResponse {
    let options = state.log_store.filter_options().await;
    (StatusCode::OK, Json(options))
}
