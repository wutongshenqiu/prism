pub mod claude;
pub mod common;
pub mod gemini;
pub mod openai;
pub mod openai_compat;
pub mod routing;
pub mod sse;

use prism_core::provider::{Format, ProviderExecutor};
use prism_core::proxy::HttpClientPool;
use std::collections::HashMap;
use std::sync::Arc;

/// Extract response headers from a reqwest Response into a HashMap.
pub fn extract_headers(resp: &reqwest::Response) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    for (key, value) in resp.headers().iter() {
        if let Ok(v) = value.to_str() {
            headers.insert(key.as_str().to_string(), v.to_string());
        }
    }
    headers
}

/// Parse the `Retry-After` header value as seconds.
/// Handles integer seconds only (ignores HTTP-date format).
pub fn parse_retry_after(headers: &HashMap<String, String>) -> Option<u64> {
    headers
        .get("retry-after")
        .and_then(|v| v.parse::<u64>().ok())
}

pub struct ExecutorRegistry {
    executors: HashMap<String, Arc<dyn ProviderExecutor>>,
}

impl ExecutorRegistry {
    pub fn get(&self, name: &str) -> Option<Arc<dyn ProviderExecutor>> {
        self.executors.get(name).cloned()
    }

    pub fn get_by_format(&self, format: Format) -> Option<Arc<dyn ProviderExecutor>> {
        self.executors
            .values()
            .find(|e| e.native_format() == format)
            .cloned()
    }

    pub fn all(&self) -> impl Iterator<Item = (&String, &Arc<dyn ProviderExecutor>)> {
        self.executors.iter()
    }
}

pub fn build_registry(
    global_proxy: Option<String>,
    client_pool: Arc<HttpClientPool>,
) -> ExecutorRegistry {
    let mut executors: HashMap<String, Arc<dyn ProviderExecutor>> = HashMap::new();

    // OpenAI executor (OpenAI-compatible with OpenAI defaults)
    let openai = openai::new_openai_executor(global_proxy.clone(), client_pool.clone());
    executors.insert("openai".to_string(), Arc::new(openai));

    // Claude executor
    let claude = claude::ClaudeExecutor::new(global_proxy.clone(), client_pool.clone());
    executors.insert("claude".to_string(), Arc::new(claude));

    // Gemini executor
    let gemini = gemini::GeminiExecutor::new(global_proxy.clone(), client_pool.clone());
    executors.insert("gemini".to_string(), Arc::new(gemini));

    // OpenAI-compatible generic executor (no default base_url - users must provide base-url in config)
    let compat = openai_compat::OpenAICompatExecutor {
        name: "openai-compat".to_string(),
        default_base_url: String::new(),
        format: Format::OpenAICompat,
        global_proxy: global_proxy.clone(),
        client_pool,
    };
    executors.insert("openai-compat".to_string(), Arc::new(compat));

    ExecutorRegistry { executors }
}
