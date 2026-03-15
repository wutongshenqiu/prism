use crate::sse::parse_sse_stream;
use prism_core::auth_profile::AuthHeaderKind;
use prism_core::error::ProxyError;
use prism_core::presentation::protected::is_protected;
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
        if is_protected(k) {
            continue;
        }
        req = req.header(k.as_str(), v.as_str());
    }
    for (k, v) in &auth.headers {
        if is_protected(k) {
            continue;
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use prism_core::auth_profile::AuthMode;
    use prism_core::circuit_breaker::NoopCircuitBreaker;
    use std::sync::Arc;

    fn make_auth() -> AuthRecord {
        AuthRecord {
            id: "auth-1".into(),
            provider: Format::OpenAI,
            provider_name: "openai".into(),
            api_key: "secret".into(),
            base_url: None,
            proxy_url: None,
            headers: HashMap::from([
                ("authorization".into(), "Bearer evil".into()),
                ("x-custom".into(), "ok".into()),
            ]),
            models: Vec::new(),
            excluded_models: Vec::new(),
            prefix: None,
            disabled: false,
            circuit_breaker: Arc::new(NoopCircuitBreaker),
            cloak: None,
            wire_api: Default::default(),
            credential_name: None,
            auth_profile_id: "default".into(),
            auth_mode: AuthMode::ApiKey,
            auth_header: AuthHeaderKind::Bearer,
            oauth_state: None,
            weight: 1,
            region: None,
            upstream_presentation: Default::default(),
            vertex: false,
            vertex_project: None,
            vertex_location: None,
        }
    }

    #[test]
    fn test_apply_headers_skips_protected_headers() {
        let client = reqwest::Client::new();
        let request = client.get("https://example.com");
        let mut request_headers = HashMap::new();
        request_headers.insert("x-api-key".into(), "evil".into());
        request_headers.insert("x-test".into(), "safe".into());

        let built = apply_headers(request, &request_headers, &make_auth())
            .build()
            .expect("build request");
        let headers = built.headers();
        assert_eq!(headers.get("x-test").unwrap(), "safe");
        assert_eq!(headers.get("x-custom").unwrap(), "ok");
        assert!(headers.get("authorization").is_none());
        assert!(headers.get("x-api-key").is_none());
    }
}
