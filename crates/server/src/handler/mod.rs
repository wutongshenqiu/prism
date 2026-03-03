pub mod admin;
pub mod chat_completions;
pub mod dashboard;
pub mod health;
pub mod messages;
pub mod models;
pub mod responses;

use crate::AppState;
use crate::dispatch::{DispatchRequest, dispatch};
use axum::http::HeaderMap;
use axum::response::Response;
use bytes::Bytes;
use prism_core::context::RequestContext;
use prism_core::error::ProxyError;
use prism_core::provider::Format;

#[derive(Debug)]
pub(crate) struct ParsedRequest {
    pub model: String,
    /// Fallback model chain: try models in order until one succeeds.
    pub models: Option<Vec<String>>,
    pub stream: bool,
    pub user_agent: Option<String>,
    /// Debug mode: return routing details in response headers.
    pub debug: bool,
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

    // Parse `models` array for fallback chain
    let models = req_value.get("models").and_then(|v| {
        v.as_array().map(|arr| {
            arr.iter()
                .filter_map(|m| m.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
    });

    let stream = req_value
        .get("stream")
        .and_then(|s| s.as_bool())
        .unwrap_or(false);

    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Check x-debug header
    let debug = headers
        .get("x-debug")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v == "true" || v == "1");

    Ok(ParsedRequest {
        model,
        models,
        stream,
        user_agent,
        debug,
    })
}

/// Shared dispatch logic for chat_completions and messages handlers.
pub(crate) async fn dispatch_api_request(
    state: &AppState,
    ctx: &RequestContext,
    headers: &HeaderMap,
    body: Bytes,
    source_format: Format,
    allowed_formats: Option<Vec<Format>>,
) -> Result<Response, ProxyError> {
    let parsed = parse_request(headers, &body)?;

    dispatch(
        state,
        DispatchRequest {
            source_format,
            model: parsed.model,
            models: parsed.models,
            stream: parsed.stream,
            body,
            allowed_formats,
            user_agent: parsed.user_agent,
            debug: parsed.debug,
            api_key: ctx.auth_key.as_ref().map(|e| e.key.clone()),
            client_region: ctx.client_region.clone(),
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;
    use bytes::Bytes;

    fn make_body(json: serde_json::Value) -> Bytes {
        Bytes::from(serde_json::to_vec(&json).unwrap())
    }

    #[test]
    fn test_parse_request_basic() {
        let body = make_body(serde_json::json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "hi"}]
        }));
        let headers = HeaderMap::new();
        let parsed = parse_request(&headers, &body).unwrap();
        assert_eq!(parsed.model, "gpt-4");
        assert!(!parsed.stream);
        assert!(!parsed.debug);
        assert!(parsed.models.is_none());
        assert!(parsed.user_agent.is_none());
    }

    #[test]
    fn test_parse_request_with_stream() {
        let body = make_body(serde_json::json!({
            "model": "gpt-4",
            "stream": true
        }));
        let headers = HeaderMap::new();
        let parsed = parse_request(&headers, &body).unwrap();
        assert!(parsed.stream);
    }

    #[test]
    fn test_parse_request_with_models_fallback() {
        let body = make_body(serde_json::json!({
            "model": "gpt-4",
            "models": ["gpt-4", "gpt-3.5-turbo", "claude-3-sonnet"]
        }));
        let headers = HeaderMap::new();
        let parsed = parse_request(&headers, &body).unwrap();
        assert_eq!(
            parsed.models,
            Some(vec![
                "gpt-4".to_string(),
                "gpt-3.5-turbo".to_string(),
                "claude-3-sonnet".to_string()
            ])
        );
    }

    #[test]
    fn test_parse_request_user_agent() {
        let body = make_body(serde_json::json!({"model": "gpt-4"}));
        let mut headers = HeaderMap::new();
        headers.insert("user-agent", "opencode/1.0".parse().unwrap());
        let parsed = parse_request(&headers, &body).unwrap();
        assert_eq!(parsed.user_agent, Some("opencode/1.0".to_string()));
    }

    #[test]
    fn test_parse_request_debug_true() {
        let body = make_body(serde_json::json!({"model": "gpt-4"}));
        let mut headers = HeaderMap::new();
        headers.insert("x-debug", "true".parse().unwrap());
        let parsed = parse_request(&headers, &body).unwrap();
        assert!(parsed.debug);
    }

    #[test]
    fn test_parse_request_debug_one() {
        let body = make_body(serde_json::json!({"model": "gpt-4"}));
        let mut headers = HeaderMap::new();
        headers.insert("x-debug", "1".parse().unwrap());
        let parsed = parse_request(&headers, &body).unwrap();
        assert!(parsed.debug);
    }

    #[test]
    fn test_parse_request_debug_false_value() {
        let body = make_body(serde_json::json!({"model": "gpt-4"}));
        let mut headers = HeaderMap::new();
        headers.insert("x-debug", "false".parse().unwrap());
        let parsed = parse_request(&headers, &body).unwrap();
        assert!(!parsed.debug);
    }

    #[test]
    fn test_parse_request_missing_model() {
        let body = make_body(serde_json::json!({"messages": []}));
        let headers = HeaderMap::new();
        let err = parse_request(&headers, &body).unwrap_err();
        assert!(err.to_string().contains("missing model"));
    }

    #[test]
    fn test_parse_request_invalid_json() {
        let body = Bytes::from_static(b"not json");
        let headers = HeaderMap::new();
        let err = parse_request(&headers, &body).unwrap_err();
        assert!(matches!(err, ProxyError::BadRequest(_)));
    }

    #[test]
    fn test_parse_request_stream_default_false() {
        let body = make_body(serde_json::json!({"model": "m"}));
        let headers = HeaderMap::new();
        let parsed = parse_request(&headers, &body).unwrap();
        assert!(!parsed.stream);
    }

    #[test]
    fn test_parse_request_models_non_array_ignored() {
        let body = make_body(serde_json::json!({
            "model": "gpt-4",
            "models": "not-an-array"
        }));
        let headers = HeaderMap::new();
        let parsed = parse_request(&headers, &body).unwrap();
        assert!(parsed.models.is_none());
    }
}
