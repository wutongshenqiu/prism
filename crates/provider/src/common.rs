use crate::sse::parse_sse_stream;
use prism_core::auth_profile::AuthHeaderKind;
use prism_core::error::ProxyError;
use prism_core::provider::*;
use prism_core::proxy::HttpClientPool;
use std::collections::HashMap;

/// Build an HTTP client for a provider request using a shared pool.
pub fn build_client(
    auth: &AuthRecord,
    global_proxy: Option<&str>,
    pool: &HttpClientPool,
) -> Result<reqwest::Client, ProxyError> {
    pool.get_or_create_default(auth.effective_proxy(global_proxy), global_proxy)
        .map_err(|e| ProxyError::Internal(format!("failed to build HTTP client: {e}")))
}

/// Apply request-level and per-credential headers to a request builder.
pub fn apply_headers(
    mut req: reqwest::RequestBuilder,
    request_headers: &HashMap<String, String>,
    auth: &AuthRecord,
) -> reqwest::RequestBuilder {
    for (k, v) in request_headers {
        req = req.header(k.as_str(), v.as_str());
    }
    for (k, v) in &auth.headers {
        req = req.header(k.as_str(), v.as_str());
    }
    req
}

/// Apply the resolved auth header to a request builder.
pub fn apply_auth(mut req: reqwest::RequestBuilder, auth: &AuthRecord) -> reqwest::RequestBuilder {
    let secret = auth.current_secret();
    match auth.resolved_auth_header_kind() {
        AuthHeaderKind::Bearer | AuthHeaderKind::Auto => {
            req = req.header("authorization", format!("Bearer {}", secret));
        }
        AuthHeaderKind::XApiKey => {
            req = req.header("x-api-key", secret);
        }
        AuthHeaderKind::XGoogApiKey => {
            req = req.header("x-goog-api-key", secret);
        }
    }
    req
}

/// Handle a non-streaming response: check status, extract body and headers.
pub async fn handle_response(
    resp: reqwest::Response,
) -> Result<(bytes::Bytes, HashMap<String, String>), ProxyError> {
    let status = resp.status().as_u16();
    let headers = crate::extract_headers(&resp);
    let body = resp.bytes().await?;

    if status >= 400 {
        return Err(ProxyError::Upstream {
            status,
            body: String::from_utf8_lossy(&body).to_string(),
            retry_after_secs: crate::parse_retry_after(&headers),
        });
    }

    Ok((body, headers))
}

/// Handle a streaming response: check status, parse SSE stream.
pub async fn handle_stream_response(resp: reqwest::Response) -> Result<StreamResult, ProxyError> {
    let status = resp.status().as_u16();
    let headers = crate::extract_headers(&resp);

    if status >= 400 {
        let body = resp.bytes().await?;
        return Err(ProxyError::Upstream {
            status,
            body: String::from_utf8_lossy(&body).to_string(),
            retry_after_secs: crate::parse_retry_after(&headers),
        });
    }

    let byte_stream = resp.bytes_stream();
    let sse_stream = parse_sse_stream(byte_stream);

    let chunk_stream = tokio_stream::StreamExt::map(sse_stream, |result| {
        result.map(|event| StreamChunk {
            event_type: event.event,
            data: event.data,
        })
    });

    Ok(StreamResult {
        headers,
        stream: Box::pin(chunk_stream),
    })
}

/// Build a model list from an auth record's configured models.
pub fn supported_models_from_auth(
    auth: &AuthRecord,
    provider: &str,
    owned_by: &str,
) -> Vec<ModelInfo> {
    auth.models
        .iter()
        .filter(|m| auth.supports_model(&m.id))
        .map(|m| {
            let id = m.alias.as_deref().unwrap_or(&m.id);
            ModelInfo {
                id: id.to_string(),
                provider: provider.to_string(),
                owned_by: owned_by.to_string(),
            }
        })
        .collect()
}
