use crate::AppState;
use ai_proxy_core::context::RequestContext;
use ai_proxy_core::error::ProxyError;
use ai_proxy_core::provider::Format;
use axum::Extension;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;
use bytes::Bytes;

/// Claude Messages API passthrough (/v1/messages).
pub async fn messages(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    super::dispatch_api_request(
        &state,
        &ctx,
        &headers,
        body,
        Format::Claude,
        Some(vec![Format::Claude]),
    )
    .await
}
