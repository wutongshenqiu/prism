pub mod admin;
pub mod chat_completions;
pub mod dashboard;
pub mod health;
pub mod messages;
pub mod models;
pub mod responses;

use ai_proxy_core::error::ProxyError;
use axum::http::HeaderMap;
use bytes::Bytes;

pub(crate) struct ParsedRequest {
    pub model: String,
    pub stream: bool,
    pub user_agent: Option<String>,
}

pub(crate) fn parse_request(
    headers: &HeaderMap,
    body: &Bytes,
) -> Result<ParsedRequest, ProxyError> {
    let req_value: serde_json::Value =
        serde_json::from_slice(body).map_err(|e| ProxyError::BadRequest(e.to_string()))?;

    let model = req_value
        .get("model")
        .and_then(|m| m.as_str())
        .ok_or_else(|| ProxyError::BadRequest("missing model field".into()))?
        .to_string();

    let stream = req_value
        .get("stream")
        .and_then(|s| s.as_bool())
        .unwrap_or(false);

    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    Ok(ParsedRequest {
        model,
        stream,
        user_agent,
    })
}
