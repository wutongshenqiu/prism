use axum::extract::ConnectInfo;
use axum::{extract::Request, middleware::Next, response::Response};
use prism_core::context::RequestContext;
use std::net::SocketAddr;

/// Middleware that injects a `RequestContext` as an axum Extension.
///
/// Client IP is derived from the socket peer address by default.
/// Forwarded headers (`X-Forwarded-For`, `X-Real-IP`) are NOT trusted
/// unless a reverse proxy is explicitly configured, preventing IP spoofing
/// that could bypass `localhost_only` or login rate limiting.
pub async fn request_context_middleware(mut request: Request, next: Next) -> Response {
    // Use the actual socket peer address as the client IP (safe default)
    let client_ip = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ci| ci.0.ip().to_string());

    // Extract client region from CDN / custom headers
    let client_region = request
        .headers()
        .get("x-client-region")
        .or_else(|| request.headers().get("cf-ipcountry"))
        .or_else(|| request.headers().get("x-vercel-ip-country"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let mut ctx = RequestContext::new(client_ip);
    ctx.client_region = client_region;
    request.extensions_mut().insert(ctx);
    next.run(request).await
}
