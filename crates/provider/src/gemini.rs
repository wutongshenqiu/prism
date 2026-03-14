use crate::common;
use async_trait::async_trait;
use prism_core::error::ProxyError;
use prism_core::provider::*;
use prism_core::proxy::HttpClientPool;
use std::sync::Arc;

const DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com";

pub struct GeminiExecutor {
    pub global_proxy: Option<String>,
    pub client_pool: Arc<HttpClientPool>,
}

impl GeminiExecutor {
    pub fn new(global_proxy: Option<String>, client_pool: Arc<HttpClientPool>) -> Self {
        Self {
            global_proxy,
            client_pool,
        }
    }

    /// Build a POST request with Gemini-specific auth header.
    /// For Vertex AI credentials, uses Bearer token auth instead of x-goog-api-key.
    fn build_request(
        &self,
        auth: &AuthRecord,
        url: &str,
        request: &ProviderRequest,
    ) -> Result<reqwest::RequestBuilder, ProxyError> {
        let client = common::build_client(auth, self.global_proxy.as_deref(), &self.client_pool)?;

        let req = client.post(url).header("content-type", "application/json");

        let req = if auth.vertex {
            req.header("authorization", format!("Bearer {}", auth.api_key))
        } else {
            req.header("x-goog-api-key", &auth.api_key)
        };

        let req = req.body(request.payload.to_vec());
        Ok(common::apply_headers(req, &request.headers, auth))
    }

    /// Construct the URL for a Gemini/Vertex API call.
    fn build_url(&self, auth: &AuthRecord, model: &str, stream: bool) -> String {
        if auth.vertex {
            let base_url = auth
                .base_url
                .as_deref()
                .unwrap_or("https://us-central1-aiplatform.googleapis.com");
            let base_url = base_url.trim_end_matches('/');
            let project = auth.vertex_project.as_deref().unwrap_or("default");
            let location = auth.vertex_location.as_deref().unwrap_or("us-central1");
            let action = if stream {
                "streamGenerateContent"
            } else {
                "generateContent"
            };
            format!(
                "{base_url}/v1/projects/{project}/locations/{location}/publishers/google/models/{model}:{action}"
            )
        } else {
            let base_url = auth.base_url_or_default(DEFAULT_BASE_URL);
            if stream {
                format!("{base_url}/v1beta/models/{model}:streamGenerateContent?alt=sse")
            } else {
                format!("{base_url}/v1beta/models/{model}:generateContent")
            }
        }
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
        let url = self.build_url(auth, &request.model, false);
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
        let mut url = self.build_url(auth, &request.model, true);
        // Vertex AI streaming requires alt=sse; standard Gemini already includes it
        if auth.vertex && !url.contains("alt=sse") {
            url.push_str("?alt=sse");
        }
        let req = self.build_request(auth, &url, &request)?;

        common::handle_stream_response(req.send().await?).await
    }

    fn supported_models(&self, auth: &AuthRecord) -> Vec<ModelInfo> {
        let provider = if auth.vertex { "vertex" } else { "gemini" };
        common::supported_models_from_auth(auth, provider, "google")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prism_core::circuit_breaker::NoopCircuitBreaker;

    fn make_gemini_auth() -> AuthRecord {
        AuthRecord {
            id: "test-gemini".to_string(),
            provider: Format::Gemini,
            api_key: "AIzaSyTest".to_string(),
            base_url: None,
            proxy_url: None,
            headers: Default::default(),
            models: vec![],
            excluded_models: vec![],
            prefix: None,
            disabled: false,
            circuit_breaker: Arc::new(NoopCircuitBreaker),
            cloak: None,
            wire_api: Default::default(),
            credential_name: None,
            weight: 1,
            region: None,
            vertex: false,
            vertex_project: None,
            vertex_location: None,
        }
    }

    fn make_vertex_auth() -> AuthRecord {
        let mut auth = make_gemini_auth();
        auth.api_key = "ya29.access-token".to_string();
        auth.vertex = true;
        auth.vertex_project = Some("my-project".to_string());
        auth.vertex_location = Some("us-central1".to_string());
        auth
    }

    #[test]
    fn test_gemini_url_non_stream() {
        let exec = GeminiExecutor::new(None, Arc::new(HttpClientPool::new()));
        let auth = make_gemini_auth();
        let url = exec.build_url(&auth, "gemini-2.0-flash", false);
        assert_eq!(
            url,
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent"
        );
    }

    #[test]
    fn test_gemini_url_stream() {
        let exec = GeminiExecutor::new(None, Arc::new(HttpClientPool::new()));
        let auth = make_gemini_auth();
        let url = exec.build_url(&auth, "gemini-2.0-flash", true);
        assert_eq!(
            url,
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:streamGenerateContent?alt=sse"
        );
    }

    #[test]
    fn test_gemini_url_custom_base() {
        let exec = GeminiExecutor::new(None, Arc::new(HttpClientPool::new()));
        let mut auth = make_gemini_auth();
        auth.base_url = Some("https://custom.api.example.com".to_string());
        let url = exec.build_url(&auth, "gemini-1.5-pro", false);
        assert_eq!(
            url,
            "https://custom.api.example.com/v1beta/models/gemini-1.5-pro:generateContent"
        );
    }

    #[test]
    fn test_vertex_url_non_stream() {
        let exec = GeminiExecutor::new(None, Arc::new(HttpClientPool::new()));
        let auth = make_vertex_auth();
        let url = exec.build_url(&auth, "gemini-2.0-flash", false);
        assert_eq!(
            url,
            "https://us-central1-aiplatform.googleapis.com/v1/projects/my-project/locations/us-central1/publishers/google/models/gemini-2.0-flash:generateContent"
        );
    }

    #[test]
    fn test_vertex_url_stream() {
        let exec = GeminiExecutor::new(None, Arc::new(HttpClientPool::new()));
        let auth = make_vertex_auth();
        let url = exec.build_url(&auth, "gemini-2.0-flash", true);
        assert_eq!(
            url,
            "https://us-central1-aiplatform.googleapis.com/v1/projects/my-project/locations/us-central1/publishers/google/models/gemini-2.0-flash:streamGenerateContent"
        );
    }

    #[test]
    fn test_vertex_url_custom_base() {
        let exec = GeminiExecutor::new(None, Arc::new(HttpClientPool::new()));
        let mut auth = make_vertex_auth();
        auth.base_url = Some("https://europe-west1-aiplatform.googleapis.com".to_string());
        auth.vertex_location = Some("europe-west1".to_string());
        let url = exec.build_url(&auth, "gemini-1.5-pro", false);
        assert_eq!(
            url,
            "https://europe-west1-aiplatform.googleapis.com/v1/projects/my-project/locations/europe-west1/publishers/google/models/gemini-1.5-pro:generateContent"
        );
    }

    #[test]
    fn test_vertex_supported_models_provider_name() {
        let exec = GeminiExecutor::new(None, Arc::new(HttpClientPool::new()));
        let mut auth = make_vertex_auth();
        auth.models = vec![ModelEntry {
            id: "gemini-2.0-flash".to_string(),
            alias: None,
        }];
        let models = exec.supported_models(&auth);
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].provider, "vertex");
    }
}
