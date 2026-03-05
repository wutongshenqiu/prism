use crate::common;
use async_trait::async_trait;
use prism_core::error::ProxyError;
use prism_core::provider::*;

const DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com";

pub struct GeminiExecutor {
    pub global_proxy: Option<String>,
}

impl GeminiExecutor {
    pub fn new(global_proxy: Option<String>) -> Self {
        Self { global_proxy }
    }

    /// Build a POST request with Gemini-specific auth header.
    fn build_request(
        &self,
        auth: &AuthRecord,
        url: &str,
        request: &ProviderRequest,
    ) -> Result<reqwest::RequestBuilder, ProxyError> {
        let client = common::build_client(auth, self.global_proxy.as_deref())?;

        let req = client
            .post(url)
            .header("content-type", "application/json")
            .header("x-goog-api-key", &auth.api_key)
            .body(request.payload.to_vec());

        Ok(common::apply_headers(req, &request.headers, auth))
    }
}

#[async_trait]
impl ProviderExecutor for GeminiExecutor {
    fn identifier(&self) -> &str {
        "gemini"
    }

    fn native_format(&self) -> Format {
        Format::Gemini
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
        let url = format!("{base_url}/v1beta/models/{}:generateContent", request.model);
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
        let url = format!(
            "{base_url}/v1beta/models/{}:streamGenerateContent?alt=sse",
            request.model
        );
        let req = self.build_request(auth, &url, &request)?;

        common::handle_stream_response(req.send().await?).await
    }

    fn supported_models(&self, auth: &AuthRecord) -> Vec<ModelInfo> {
        common::supported_models_from_auth(auth, "gemini", "google")
    }
}
