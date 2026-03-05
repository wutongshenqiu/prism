use crate::AppState;
use crate::dispatch::DispatchMeta;
use axum::extract::State;
use axum::{extract::Request, middleware::Next, response::Response};
use prism_core::audit::AuditEntry;
use prism_core::context::RequestContext;
use prism_core::request_log::RequestLogEntry;

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
        // Read dispatch metadata from response extensions (set by dispatch)
        let meta = response.extensions().get::<DispatchMeta>().cloned();

        let api_key_id = ctx.as_ref().and_then(|c| c.api_key_id.clone());
        let tenant_id = ctx.as_ref().and_then(|c| c.tenant_id.clone());
        let client_ip_log = ctx.as_ref().and_then(|c| c.client_ip.clone());

        let entry = RequestLogEntry {
            timestamp: chrono::Utc::now().timestamp_millis(),
            request_id: request_id.clone(),
            method: method.to_string(),
            path: uri.clone(),
            status,
            latency_ms: elapsed as u64,
            provider: meta.as_ref().and_then(|m| m.provider.clone()),
            model: meta.as_ref().and_then(|m| m.model.clone()),
            input_tokens: meta.as_ref().and_then(|m| m.input_tokens),
            output_tokens: meta.as_ref().and_then(|m| m.output_tokens),
            cost: meta.as_ref().and_then(|m| m.cost),
            error: if status >= 400 {
                Some(format!("HTTP {status}"))
            } else {
                None
            },
            api_key_id: api_key_id.clone(),
            tenant_id: tenant_id.clone(),
            client_ip: client_ip_log.clone(),
        };
        state.request_logs.push(entry);

        // Write audit entry
        let audit_entry = AuditEntry {
            timestamp: chrono::Utc::now(),
            request_id,
            method: method.to_string(),
            path: uri,
            status,
            latency_ms: elapsed as u64,
            provider: meta.as_ref().and_then(|m| m.provider.clone()),
            model: meta.as_ref().and_then(|m| m.model.clone()),
            input_tokens: meta.as_ref().and_then(|m| m.input_tokens),
            output_tokens: meta.as_ref().and_then(|m| m.output_tokens),
            cost: meta.as_ref().and_then(|m| m.cost),
            error: if status >= 400 {
                Some(format!("HTTP {status}"))
            } else {
                None
            },
            api_key_id,
            tenant_id,
            client_ip: client_ip_log,
        };
        let audit = state.audit.clone();
        tokio::spawn(async move {
            audit.write(&audit_entry).await;
            // Note: audit.write() is fire-and-forget by trait design.
            // Errors are logged within the backend implementation.
        });
    }

    response
}
