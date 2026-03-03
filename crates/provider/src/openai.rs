use crate::openai_compat::OpenAICompatExecutor;
use prism_core::provider::Format;

pub fn new_openai_executor(global_proxy: Option<String>) -> OpenAICompatExecutor {
    OpenAICompatExecutor {
        name: "openai".to_string(),
        default_base_url: "https://api.openai.com".to_string(),
        format: Format::OpenAI,
        global_proxy,
    }
}
