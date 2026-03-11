use crate::AppState;
use crate::dispatch::DispatchMeta;
use axum::extract::State;
use axum::{extract::Request, middleware::Next, response::Response};
use prism_core::context::RequestContext;
use prism_core::request_record::RequestRecord;

/// Middleware that logs request/response with request context info.
/// Also captures proxy requests (/v1/*) into the request log ring buffer
/// and writes audit entries.
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
        let meta = response.extensions().get::<DispatchMeta>().cloned();

        let record = RequestRecord {
            request_id: request_id.clone(),
            timestamp: chrono::Utc::now(),
            method: method.to_string(),
            path: uri,
            stream: meta.as_ref().is_some_and(|m| m.stream),
            requested_model: meta.as_ref().and_then(|m| m.requested_model.clone()),
            provider: meta.as_ref().and_then(|m| m.provider.clone()),
            model: meta.as_ref().and_then(|m| m.model.clone()),
            credential_name: meta.as_ref().and_then(|m| m.credential_name.clone()),
            retry_count: meta.as_ref().map(|m| m.retry_count).unwrap_or(0),
            status,
            latency_ms: elapsed as u64,
            usage: meta.as_ref().and_then(|m| m.usage.clone()),
            cost: meta.as_ref().and_then(|m| m.cost),
            error: if status >= 400 {
                meta.as_ref()
                    .and_then(|m| m.error_detail.clone())
                    .or_else(|| Some(format!("HTTP {status}")))
            } else {
                None
            },
            api_key_id: ctx.as_ref().and_then(|c| c.api_key_id.clone()),
            tenant_id: ctx.as_ref().and_then(|c| c.tenant_id.clone()),
            client_ip: ctx.as_ref().and_then(|c| c.client_ip.clone()),
        };
        state.request_logs.push(record.clone());

        // Write audit entry
        let audit = state.audit.clone();
        tokio::spawn(async move {
            audit.write(&record).await;
        });
    }

    response
}
