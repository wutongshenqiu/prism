use crate::AppState;
use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;

pub async fn health() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

pub async fn metrics(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.metrics.snapshot())
}

/// GET /metrics/prometheus — Prometheus text exposition format.
pub async fn prometheus_metrics(State(state): State<AppState>) -> impl IntoResponse {
    let cache_stats = state.response_cache.as_ref().map(|c| c.stats());

    let cb_states = state.router.circuit_breaker_states();

    let body =
        prism_core::prometheus::render_metrics(&state.metrics, cache_stats.as_ref(), &cb_states);

    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    )
}
