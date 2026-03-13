use crate::openai_compat::OpenAICompatExecutor;
use prism_core::provider::Format;
use prism_core::proxy::HttpClientPool;
use std::sync::Arc;

pub fn new_openai_executor(
    global_proxy: Option<String>,
    client_pool: Arc<HttpClientPool>,
) -> OpenAICompatExecutor {
    OpenAICompatExecutor {
        name: "openai".to_string(),
        default_base_url: "https://api.openai.com".to_string(),
        format: Format::OpenAI,
        global_proxy,
        client_pool,
    }
}
