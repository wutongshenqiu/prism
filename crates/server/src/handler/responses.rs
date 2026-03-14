use crate::AppState;
use crate::dispatch::{DispatchRequest, dispatch};
use axum::Extension;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;
use bytes::Bytes;
use prism_core::context::RequestContext;
use prism_core::error::ProxyError;
use prism_core::provider::Format;

/// OpenAI Responses API (/v1/responses).
/// Routes through the unified dispatch pipeline with responses_passthrough=true
/// so the executor forwards the body directly to upstream /v1/responses.
pub async fn responses(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    let parsed = super::parse_request(&headers, &body)?;

    let allowed_credentials = ctx
        .auth_key
        .as_ref()
        .map(|e| e.allowed_credentials.clone())
        .unwrap_or_default();

    dispatch(
        &state,
        DispatchRequest {
            source_format: Format::OpenAI,
            model: parsed.model,
            models: parsed.models,
            stream: parsed.stream,
            body,
            allowed_formats: Some(vec![Format::OpenAI]),
            user_agent: parsed.user_agent,
            debug: parsed.debug,
            api_key: ctx.auth_key.as_ref().map(|e| e.key.clone()),
            client_region: ctx.client_region.clone(),
            request_id: Some(ctx.request_id.clone()),
            api_key_id: ctx.api_key_id.clone(),
            tenant_id: ctx.tenant_id.clone(),
            allowed_credentials,
            responses_passthrough: true,
        },
    )
    .await
}
