use crate::common;
use async_trait::async_trait;
use prism_core::error::ProxyError;
use prism_core::provider::*;
use prism_core::proxy::HttpClientPool;
use std::sync::Arc;

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const ANTHROPIC_BETA: &str = "output-128k-2025-02-19";

pub struct ClaudeExecutor {
    pub global_proxy: Option<String>,
    pub client_pool: Arc<HttpClientPool>,
}

impl ClaudeExecutor {
    pub fn new(global_proxy: Option<String>, client_pool: Arc<HttpClientPool>) -> Self {
        Self {
            global_proxy,
            client_pool,
        }
    }

    /// Build a POST request with Claude-specific auth and version headers.
    fn build_request(
        &self,
        auth: &AuthRecord,
        url: &str,
        request: &ProviderRequest,
    ) -> Result<reqwest::RequestBuilder, ProxyError> {
        let client = common::build_client(auth, self.global_proxy.as_deref(), &self.client_pool)?;

        let mut req = client
            .post(url)
            .header("content-type", "application/json")
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("anthropic-beta", ANTHROPIC_BETA);

        // Determine whether to use x-api-key (for anthropic.com) or Bearer auth.
        let base_url = auth.base_url_or_default(DEFAULT_BASE_URL);
        if base_url.contains("anthropic.com") {
            req = req.header("x-api-key", &auth.api_key);
        } else {
            req = req.header("authorization", format!("Bearer {}", auth.api_key));
        }

        req = common::apply_headers(req, &request.headers, auth);
        Ok(req.body(request.payload.to_vec()))
    }
}

#[async_trait]
impl ProviderExecutor for ClaudeExecutor {
    fn identifier(&self) -> &str {
        "claude"
    }

    fn native_format(&self) -> Format {
        Format::Claude
    }

    fn default_base_url(&self) -> &str {
        DEFAULT_BASE_URL
    }

    async fn execute(
        &self,
        auth: &AuthRecord,
        request: ProviderRequest,
    ) -> Result<ProviderResponse, ProxyError> {
        let base_url = auth.base_url_or_default(DEFAULT_BASE_URL);
        let url = format!("{base_url}/v1/messages");
        let req = self.build_request(auth, &url, &request)?;

        let (body, headers) = common::handle_response(req.send().await?).await?;
        Ok(ProviderResponse {
            payload: body,
            headers,
        })
    }

    async fn execute_stream(
        &self,
        auth: &AuthRecord,
        request: ProviderRequest,
    ) -> Result<StreamResult, ProxyError> {
        let base_url = auth.base_url_or_default(DEFAULT_BASE_URL);
        let url = format!("{base_url}/v1/messages");
        let req = self.build_request(auth, &url, &request)?;

        common::handle_stream_response(req.send().await?).await
    }

    fn supported_models(&self, auth: &AuthRecord) -> Vec<ModelInfo> {
        common::supported_models_from_auth(auth, "claude", "anthropic")
    }
}
