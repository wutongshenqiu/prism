use crate::AppState;
use axum::Extension;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;
use bytes::Bytes;
use prism_core::context::RequestContext;
use prism_core::error::ProxyError;
use prism_core::provider::Format;

/// POST /v1/completions — Legacy OpenAI Completions API.
/// Routes through the same dispatch pipeline as chat completions.
pub async fn completions(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    super::dispatch_api_request(&state, &ctx, &headers, body, Format::OpenAI, None).await
}
