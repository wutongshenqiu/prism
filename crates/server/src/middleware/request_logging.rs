use crate::AppState;
use ai_proxy_core::context::RequestContext;
use ai_proxy_core::request_log::RequestLogEntry;
use axum::extract::State;
use axum::{extract::Request, middleware::Next, response::Response};

/// Middleware that logs request/response with request context info.
/// Also captures proxy requests (/v1/*) into the request log ring buffer.
pub async fn request_logging_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let uri = request.uri().path().to_string();

    let ctx = request.extensions().get::<RequestContext>().cloned();
    let request_id = ctx
        .as_ref()
        .map(|c| c.request_id.clone())
        .unwrap_or_default();
    let client_ip = ctx
        .as_ref()
        .and_then(|c| c.client_ip.clone())
        .unwrap_or_else(|| "-".to_string());

    tracing::info!(
        request_id = %request_id,
        client_ip = %client_ip,
        method = %method,
        path = %uri,
        "Request received"
    );

    let response = next.run(request).await;

    let elapsed = ctx.as_ref().map(|c| c.elapsed_ms()).unwrap_or(0);
    let status = response.status().as_u16();

    tracing::info!(
        request_id = %request_id,
        status = status,
        elapsed_ms = elapsed,
        "Request completed"
    );

    // Capture proxy requests into the ring buffer
    if uri.starts_with("/v1/") {
        let entry = RequestLogEntry {
            timestamp: chrono::Utc::now().timestamp_millis(),
            request_id,
            method: method.to_string(),
            path: uri,
            status,
            latency_ms: elapsed as u64,
            provider: None, // Provider info is set during dispatch
            model: None,
            input_tokens: None,
            output_tokens: None,
            error: if status >= 400 {
                Some(format!("HTTP {status}"))
            } else {
                None
            },
        };
        state.request_logs.push(entry);
    }

    response
}
