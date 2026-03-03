use crate::AppState;
use axum::{extract::State, http::Request, middleware::Next, response::Response};
use prism_core::auth_key::AuthKeyStore;
use prism_core::context::RequestContext;
use prism_core::error::ProxyError;

pub async fn auth_middleware(
    State(state): State<AppState>,
    mut request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, ProxyError> {
    let config = state.config.load();

    // If no auth keys configured, skip auth
    if config.auth_keys.is_empty() {
        return Ok(next.run(request).await);
    }

    // Extract token from Authorization: Bearer or x-api-key header
    let token = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .or_else(|| {
            request
                .headers()
                .get("x-api-key")
                .and_then(|v| v.to_str().ok())
        })
        .map(|s| s.to_string());

    let token = match token {
        Some(t) => t,
        None => return Err(ProxyError::Auth("Missing API key".to_string())),
    };

    // Lookup in AuthKeyStore
    let entry = match config.auth_key_store.lookup(&token) {
        Some(e) => e,
        None => return Err(ProxyError::Auth("Invalid API key".to_string())),
    };

    // Check expiry
    if AuthKeyStore::is_expired(entry) {
        return Err(ProxyError::KeyExpired);
    }

    // Inject auth info into RequestContext
    if let Some(ctx) = request.extensions_mut().get_mut::<RequestContext>() {
        ctx.api_key_id = Some(AuthKeyStore::mask_key(&token));
        ctx.tenant_id = entry.tenant_id.clone();
        ctx.auth_key = Some(entry.clone());
    }

    Ok(next.run(request).await)
}
