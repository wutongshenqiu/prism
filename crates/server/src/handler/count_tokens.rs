use crate::AppState;
use axum::Extension;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use prism_core::context::RequestContext;
use prism_core::error::ProxyError;
use prism_core::provider::Format;

/// POST /v1/messages/count_tokens — Proxy to Claude's token counting endpoint.
pub async fn count_tokens(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    // Parse model from request body
    let req_value: serde_json::Value =
        serde_json::from_slice(&body).map_err(|e| ProxyError::BadRequest(e.to_string()))?;

    let model = req_value
        .get("model")
        .and_then(|m| m.as_str())
        .ok_or_else(|| ProxyError::BadRequest("missing model field".into()))?;

    // Enforce model ACL (same as main dispatch path)
    if let Some(ref auth_key) = ctx.auth_key
        && !prism_core::auth_key::AuthKeyStore::check_model_access(auth_key, model)
    {
        return Err(ProxyError::ModelNotAllowed(format!(
            "model '{model}' not allowed for this API key",
        )));
    }

    let allowed_credentials = ctx
        .auth_key
        .as_ref()
        .map(|e| e.allowed_credentials.clone())
        .unwrap_or_default();

    // Pick a Claude credential that supports this model
    let auth = state
        .router
        .pick(
            Format::Claude,
            model,
            &[],
            ctx.client_region.as_deref(),
            &allowed_credentials,
        )
        .ok_or_else(|| ProxyError::NoCredentials {
            provider: "claude".into(),
            model: model.to_string(),
        })?;

    // Build upstream request
    let base_url = auth
        .base_url
        .as_deref()
        .unwrap_or("https://api.anthropic.com");
    let url = format!("{base_url}/v1/messages/count_tokens");

    let global_proxy = state.config.load().proxy_url.clone();
    let client = prism_core::proxy::build_http_client(
        auth.effective_proxy(global_proxy.as_deref()),
        global_proxy.as_deref(),
    )
    .map_err(|e| ProxyError::Internal(format!("failed to build HTTP client: {e}")))?;

    let mut req = client
        .post(&url)
        .header("content-type", "application/json")
        .header("anthropic-version", "2023-06-01");

    // Auth: x-api-key for anthropic.com, Bearer for proxied endpoints
    if base_url.contains("anthropic.com") {
        req = req.header("x-api-key", &auth.api_key);
    } else {
        req = req.header("authorization", format!("Bearer {}", auth.api_key));
    }

    // Forward anthropic-beta if present in incoming headers
    if let Some(beta) = headers.get("anthropic-beta") {
        req = req.header("anthropic-beta", beta);
    }

    let resp = req
        .body(body.to_vec())
        .send()
        .await
        .map_err(|e| ProxyError::Internal(format!("upstream request failed: {e}")))?;

    let status =
        StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let resp_body = resp
        .bytes()
        .await
        .map_err(|e| ProxyError::Internal(format!("failed to read upstream response: {e}")))?;

    Ok((status, [("content-type", "application/json")], resp_body).into_response())
}
