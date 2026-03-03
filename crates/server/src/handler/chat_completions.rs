use crate::AppState;
use ai_proxy_core::context::RequestContext;
use ai_proxy_core::error::ProxyError;
use ai_proxy_core::provider::Format;
use axum::Extension;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;
use bytes::Bytes;

pub async fn chat_completions(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    super::dispatch_api_request(&state, &ctx, &headers, body, Format::OpenAI, None).await
}
