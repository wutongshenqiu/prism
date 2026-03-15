use crate::common;
use crate::openai_compat::{chat_to_responses, responses_to_chat, synthesize_chat_stream_chunks};
use crate::sse::parse_sse_stream;
use async_trait::async_trait;
use prism_core::error::ProxyError;
use prism_core::provider::*;
use prism_core::proxy::HttpClientPool;
use serde_json::json;
use std::sync::Arc;
use tokio_stream::StreamExt;

const CODEX_CLIENT_VERSION: &str = "0.101.0";
const CODEX_USER_AGENT: &str = "codex_cli_rs/0.101.0 (Mac OS 26.0.1; arm64) Apple_Terminal/464";

pub struct CodexExecutor {
    pub global_proxy: Option<String>,
    pub client_pool: Arc<HttpClientPool>,
}

impl CodexExecutor {
    pub fn new(global_proxy: Option<String>, client_pool: Arc<HttpClientPool>) -> Self {
        Self {
            global_proxy,
            client_pool,
        }
    }

    fn build_request(
        &self,
        auth: &AuthRecord,
        url: &str,
        body: &[u8],
        request_headers: &std::collections::HashMap<String, String>,
        stream: bool,
    ) -> Result<reqwest::RequestBuilder, ProxyError> {
        let client = common::build_client(auth, self.global_proxy.as_deref(), &self.client_pool)?;
        let mut req = client
            .post(url)
            .header("content-type", "application/json")
            .body(body.to_vec());
        req = common::apply_auth(req, auth);
        req = common::apply_headers(req, request_headers, auth);
        req = req
            .header(
                "accept",
                if stream {
                    "text/event-stream"
                } else {
                    "application/json"
                },
            )
            .header("connection", "keep-alive")
            .header("version", CODEX_CLIENT_VERSION)
            .header("session_id", uuid::Uuid::new_v4().to_string())
            .header("originator", "codex_cli_rs");
        if !has_explicit_user_agent(request_headers, auth) {
            req = req.header("user-agent", CODEX_USER_AGENT);
        }
        if let Some(account_id) = auth.current_account_id()
            && !account_id.trim().is_empty()
        {
            req = req.header("chatgpt-account-id", account_id);
        }
        Ok(req)
    }

    fn normalize_payload(
        &self,
        request: &ProviderRequest,
        stream: bool,
    ) -> Result<Vec<u8>, ProxyError> {
        let raw = if request.responses_passthrough {
            request.payload.to_vec()
        } else {
            chat_to_responses(&request.payload)?
        };
        let mut value: serde_json::Value =
            serde_json::from_slice(&raw).map_err(|e| ProxyError::BadRequest(e.to_string()))?;
        let obj = value
            .as_object_mut()
            .ok_or_else(|| ProxyError::BadRequest("expected JSON object".into()))?;
        obj.remove("previous_response_id");
        obj.remove("prompt_cache_retention");
        obj.remove("safety_identifier");
        obj.remove("max_output_tokens");
        if !obj.contains_key("instructions") {
            obj.insert(
                "instructions".into(),
                serde_json::Value::String(String::new()),
            );
        }
        if let Some(input) = obj.get_mut("input")
            && let Some(text) = input.as_str()
        {
            *input = json!([{
                "role": "user",
                "content": [{
                    "type": "input_text",
                    "text": text,
                }],
            }]);
        }
        obj.insert("store".into(), serde_json::Value::Bool(false));
        obj.insert("stream".into(), serde_json::Value::Bool(stream));
        serde_json::to_vec(obj).map_err(|e| ProxyError::Internal(e.to_string()))
    }

    async fn collect_completed_response(
        &self,
        resp: reqwest::Response,
    ) -> Result<ProviderResponse, ProxyError> {
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

        let mut sse_stream = parse_sse_stream(resp.bytes_stream());
        while let Some(event) = sse_stream.next().await {
            let event = event?;
            if event.data == "[DONE]" {
                continue;
            }
            let payload: serde_json::Value = serde_json::from_str(&event.data)
                .map_err(|e| ProxyError::Internal(format!("invalid Codex SSE JSON: {e}")))?;
            let event_type = payload
                .get("type")
                .and_then(|value| value.as_str())
                .or(event.event.as_deref());
            if matches!(event_type, Some("response.completed")) {
                let response = payload.get("response").cloned().ok_or_else(|| {
                    ProxyError::Internal(
                        "Codex response.completed event missing response payload".into(),
                    )
                })?;
                let body = serde_json::to_vec(&response)
                    .map_err(|e| ProxyError::Internal(e.to_string()))?;
                return Ok(ProviderResponse {
                    payload: body.into(),
                    headers,
                });
            }
        }

        Err(ProxyError::Internal(
            "Codex stream ended before response.completed".into(),
        ))
    }
}

fn has_explicit_user_agent(
    request_headers: &std::collections::HashMap<String, String>,
    auth: &AuthRecord,
) -> bool {
    request_headers
        .keys()
        .chain(auth.headers.keys())
        .any(|key| key.eq_ignore_ascii_case("user-agent"))
}

#[async_trait]
impl ProviderExecutor for CodexExecutor {
    fn identifier(&self) -> &str {
        "codex"
    }

    fn native_format(&self) -> Format {
        Format::OpenAI
    }

    async fn execute(
        &self,
        auth: &AuthRecord,
        request: ProviderRequest,
    ) -> Result<ProviderResponse, ProxyError> {
        let base_url = auth.resolved_base_url();
        let url = format!("{base_url}/responses");
        let body = self.normalize_payload(&request, true)?;
        let req = self.build_request(auth, &url, &body, &request.headers, true)?;
        let ProviderResponse {
            payload: resp_body,
            headers,
        } = self.collect_completed_response(req.send().await?).await?;
        let payload = if request.responses_passthrough {
            resp_body
        } else {
            responses_to_chat(&resp_body)?
        };
        Ok(ProviderResponse { payload, headers })
    }

    async fn execute_stream(
        &self,
        auth: &AuthRecord,
        request: ProviderRequest,
    ) -> Result<StreamResult, ProxyError> {
        if request.responses_passthrough {
            let base_url = auth.resolved_base_url();
            let url = format!("{base_url}/responses");
            let body = self.normalize_payload(&request, true)?;
            let req = self.build_request(auth, &url, &body, &request.headers, true)?;
            return common::handle_stream_response(req.send().await?).await;
        }

        // Codex chat-completions streaming is synthesized from compact responses.
        let response = self.execute(auth, request).await?;
        let v: serde_json::Value = serde_json::from_slice(&response.payload)
            .map_err(|e| ProxyError::Internal(e.to_string()))?;

        let chunks = synthesize_chat_stream_chunks(&v)?;

        Ok(StreamResult {
            headers: response.headers,
            stream: Box::pin(tokio_stream::iter(chunks)),
        })
    }

    fn supported_models(&self, auth: &AuthRecord) -> Vec<ModelInfo> {
        common::supported_models_from_auth(auth, "codex", "codex")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prism_core::auth_profile::{AuthHeaderKind, AuthMode};
    use prism_core::circuit_breaker::NoopCircuitBreaker;

    fn make_auth() -> AuthRecord {
        AuthRecord {
            id: "codex-auth".into(),
            provider: Format::OpenAI,
            upstream: UpstreamKind::Codex,
            provider_name: "codex".into(),
            api_key: "access".into(),
            base_url: None,
            proxy_url: None,
            headers: std::collections::HashMap::new(),
            models: vec![],
            excluded_models: vec![],
            prefix: None,
            disabled: false,
            circuit_breaker: Arc::new(NoopCircuitBreaker),
            cloak: None,
            wire_api: WireApi::Responses,
            credential_name: None,
            auth_profile_id: "default".into(),
            auth_mode: AuthMode::CodexOAuth,
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
    fn test_normalize_payload_injects_instructions_and_removes_stream_for_compact() {
        let exec = CodexExecutor::new(None, Arc::new(HttpClientPool::new()));
        let request = ProviderRequest {
            model: "gpt-5.4".into(),
            payload: bytes::Bytes::from_static(
                br#"{"model":"gpt-5.4","messages":[{"role":"user","content":"hi"}],"stream":true,"previous_response_id":"resp_1"}"#,
            ),
            source_format: Format::OpenAI,
            stream: false,
            headers: Default::default(),
            original_request: None,
            responses_passthrough: false,
        };
        let normalized = exec.normalize_payload(&request, true).expect("normalize");
        let value: serde_json::Value = serde_json::from_slice(&normalized).expect("json");
        assert_eq!(value["instructions"], "");
        assert_eq!(value["store"], false);
        assert_eq!(value["stream"], true);
        assert!(value.get("previous_response_id").is_none());
    }

    #[test]
    fn test_normalize_payload_coerces_string_input_and_drops_max_output_tokens() {
        let exec = CodexExecutor::new(None, Arc::new(HttpClientPool::new()));
        let request = ProviderRequest {
            model: "gpt-5.4".into(),
            payload: bytes::Bytes::from_static(
                br#"{"model":"gpt-5.4","input":"hello","max_output_tokens":64}"#,
            ),
            source_format: Format::OpenAI,
            stream: false,
            headers: Default::default(),
            original_request: None,
            responses_passthrough: true,
        };
        let normalized = exec.normalize_payload(&request, true).expect("normalize");
        let value: serde_json::Value = serde_json::from_slice(&normalized).expect("json");
        assert!(value.get("max_output_tokens").is_none());
        assert_eq!(value["input"][0]["role"], "user");
        assert_eq!(value["input"][0]["content"][0]["type"], "input_text");
        assert_eq!(value["input"][0]["content"][0]["text"], "hello");
    }

    #[test]
    fn test_build_request_adds_codex_headers() {
        let exec = CodexExecutor::new(None, Arc::new(HttpClientPool::new()));
        let req = exec
            .build_request(
                &make_auth(),
                "https://chatgpt.com/backend-api/codex/responses",
                b"{}",
                &Default::default(),
                true,
            )
            .expect("build")
            .build()
            .expect("request");
        assert_eq!(req.headers()["accept"], "text/event-stream");
        assert_eq!(req.headers()["originator"], "codex_cli_rs");
        assert_eq!(req.headers()["version"], CODEX_CLIENT_VERSION);
    }
}
