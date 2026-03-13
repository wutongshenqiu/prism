use crate::AppState;
use axum::Extension;
use axum::extract::State;
use axum::response::IntoResponse;
use bytes::Bytes;
use prism_core::context::RequestContext;
use prism_core::error::ProxyError;
use prism_core::provider::Format;

/// OpenAI Responses API passthrough (/v1/responses).
/// This endpoint forwards directly to upstream since there's no translation layer.
pub async fn responses(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    body: Bytes,
) -> Result<impl IntoResponse, ProxyError> {
    let req_value: serde_json::Value =
        serde_json::from_slice(&body).map_err(|e| ProxyError::BadRequest(e.to_string()))?;

    let model = req_value
        .get("model")
        .and_then(|m| m.as_str())
        .ok_or_else(|| ProxyError::BadRequest("missing model field".into()))?
        .to_string();

    // Enforce model ACL (same as main dispatch path)
    if let Some(ref auth_key) = ctx.auth_key
        && !prism_core::auth_key::AuthKeyStore::check_model_access(auth_key, &model)
    {
        return Err(ProxyError::ModelNotAllowed(format!(
            "model '{}' not allowed for this API key",
            model
        )));
    }

    // Resolve provider - only OpenAI(-compatible) providers support this
    let providers = state.router.resolve_providers(&model);
    let target_format = providers
        .iter()
        .find(|f| matches!(f, Format::OpenAI | Format::OpenAICompat))
        .ok_or_else(|| {
            ProxyError::BadRequest(
                "responses API only supported by OpenAI-compatible providers".into(),
            )
        })?;

    let auth = state
        .router
        .pick(
            *target_format,
            &model,
            &[],
            ctx.client_region.as_deref(),
            ctx.auth_key
                .as_ref()
                .map(|e| e.allowed_credentials.as_slice())
                .unwrap_or(&[]),
        )
        .ok_or_else(|| ProxyError::NoCredentials {
            provider: target_format.as_str().into(),
            model: model.clone(),
        })?;

    // Build a direct request to /v1/responses
    let base_url = auth.base_url_or_default("https://api.openai.com");
    let url = format!("{base_url}/v1/responses");

    let client = prism_core::proxy::build_http_client(
        auth.effective_proxy(state.config.load().proxy_url.as_deref()),
        state.config.load().proxy_url.as_deref(),
    )
    .map_err(|e| ProxyError::Internal(format!("failed to build HTTP client: {e}")))?;

    let mut req = client
        .post(&url)
        .header("authorization", format!("Bearer {}", auth.api_key))
        .header("content-type", "application/json")
        .body(body.to_vec());

    for (k, v) in &auth.headers {
        req = req.header(k.as_str(), v.as_str());
    }

    let resp = req.send().await?;
    let status = resp.status().as_u16();
    let headers = prism_provider::extract_headers(&resp);
    let resp_body = resp.bytes().await?;

    if status >= 400 {
        return Err(ProxyError::Upstream {
            status,
            body: String::from_utf8_lossy(&resp_body).to_string(),
            retry_after_secs: prism_provider::parse_retry_after(&headers),
        });
    }

    Ok((
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        resp_body.to_vec(),
    )
        .into_response())
}
