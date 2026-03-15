use arc_swap::ArcSwap;
use prism_core::config::{Config, ProviderKeyEntry};
use prism_core::cost::CostCalculator;
use prism_core::memory_log_store::InMemoryLogStore;
use prism_core::metrics::Metrics;
use prism_core::rate_limit::CompositeRateLimiter;
use prism_core::request_log::LogStore;
use prism_provider::catalog::ProviderCatalog;
use prism_provider::health::HealthManager;
use prism_provider::routing::CredentialRouter;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::net::TcpListener;

/// Skip the test if the given environment variable is not set.
/// Returns the value if present, otherwise prints a message and returns early.
macro_rules! require_env {
    ($var:expr) => {
        match std::env::var($var) {
            Ok(val) if !val.is_empty() => val,
            _ => {
                eprintln!("Skipping test: {} not set", $var);
                return;
            }
        }
    };
}
pub(crate) use require_env;

/// A test server that starts a real proxy on a random port.
pub struct TestServer {
    pub base_url: String,
    _shutdown_tx: tokio::sync::watch::Sender<bool>,
}

impl TestServer {
    /// Start a new test server with the given config.
    pub async fn start(config: Config) -> Self {
        let default_cred_strategy = config
            .routing
            .profiles
            .get(&config.routing.default_profile)
            .map(|p| p.credential_policy.strategy)
            .unwrap_or_default();
        let credential_router = Arc::new(CredentialRouter::new(default_cred_strategy));
        credential_router.update_from_config(&config);

        let catalog = Arc::new(ProviderCatalog::new());
        {
            let cred_map = credential_router.credential_map();
            catalog.update_from_credentials(&cred_map);
        }

        let http_client_pool = Arc::new(prism_core::proxy::HttpClientPool::new());
        let executors = Arc::new(prism_provider::build_registry(
            config.proxy_url.clone(),
            http_client_pool.clone(),
        ));
        let translators = Arc::new(prism_translator::build_registry());
        let rate_limiter = Arc::new(CompositeRateLimiter::new(&config.rate_limit));
        let cost_calculator = Arc::new(CostCalculator::new(&config.model_prices));
        let metrics = Arc::new(Metrics::new());
        let log_store: Arc<dyn LogStore> =
            Arc::new(InMemoryLogStore::new(config.log_store.capacity, None));

        let config = Arc::new(ArcSwap::from_pointee(config));

        let state = prism_server::AppState {
            config,
            router: credential_router,
            executors,
            translators,
            metrics,
            log_store,
            config_path: Arc::new(Mutex::new(String::new())),
            rate_limiter,
            cost_calculator,
            response_cache: None,
            thinking_cache: None,
            http_client_pool,
            start_time: Instant::now(),
            login_limiter: Arc::new(
                prism_server::handler::dashboard::auth::LoginRateLimiter::new(),
            ),
            catalog,
            health_manager: Arc::new(HealthManager::new(Default::default())),
            auth_runtime: Arc::new(prism_server::auth_runtime::AuthRuntimeManager::new()),
            oauth_sessions: Arc::new(Default::default()),
        };

        let app_router = prism_server::build_router(state);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        tokio::spawn(async move {
            let mut rx = shutdown_rx;
            let shutdown = async move {
                let _ = rx.wait_for(|v| *v).await;
            };
            axum::serve(
                listener,
                app_router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .with_graceful_shutdown(shutdown)
            .await
            .unwrap();
        });

        Self {
            base_url: format!("http://127.0.0.1:{port}"),
            _shutdown_tx: shutdown_tx,
        }
    }
}

fn make_key_entry(
    api_key: &str,
    name: &str,
    format: prism_core::provider::Format,
    base_url: Option<&str>,
) -> ProviderKeyEntry {
    ProviderKeyEntry {
        name: name.to_string(),
        format,
        api_key: api_key.to_string(),
        base_url: base_url.map(String::from),
        proxy_url: None,
        prefix: None,
        models: vec![],
        excluded_models: vec![],
        headers: HashMap::new(),
        disabled: false,
        cloak: Default::default(),
        upstream_presentation: Default::default(),
        wire_api: Default::default(),
        weight: 1,
        region: None,
        credential_source: None,
        auth_profiles: vec![],
        vertex: false,
        vertex_project: None,
        vertex_location: None,
    }
}

/// Build a config with Bailian (OpenAI-format) provider.
/// Supports both Coding Plan keys (sk-sp-xxx → coding.dashscope.aliyuncs.com)
/// and standard keys (sk-xxx → dashscope.aliyuncs.com/compatible-mode).
pub fn build_bailian_config(api_key: &str) -> Config {
    let base_url = if api_key.starts_with("sk-sp-") {
        "https://coding.dashscope.aliyuncs.com"
    } else {
        "https://dashscope.aliyuncs.com/compatible-mode"
    };
    Config {
        providers: vec![make_key_entry(
            api_key,
            "bailian",
            prism_core::provider::Format::OpenAI,
            Some(base_url),
        )],
        ..Default::default()
    }
}

/// Build a config with OpenAI provider.
pub fn build_openai_config(api_key: &str) -> Config {
    Config {
        providers: vec![make_key_entry(
            api_key,
            "openai",
            prism_core::provider::Format::OpenAI,
            None,
        )],
        ..Default::default()
    }
}

/// Build a config with Claude provider.
pub fn build_claude_config(api_key: &str) -> Config {
    Config {
        providers: vec![make_key_entry(
            api_key,
            "claude",
            prism_core::provider::Format::Claude,
            None,
        )],
        ..Default::default()
    }
}

/// Build a config with Gemini provider.
pub fn build_gemini_config(api_key: &str) -> Config {
    Config {
        providers: vec![make_key_entry(
            api_key,
            "gemini",
            prism_core::provider::Format::Gemini,
            None,
        )],
        ..Default::default()
    }
}

/// Get the Bailian model to use for tests.
/// Uses `E2E_BAILIAN_MODEL` env var if set, otherwise auto-detects:
/// - Coding Plan keys (sk-sp-xxx) → qwen3-coder-plus
/// - Standard keys (sk-xxx) → qwen-turbo-latest
pub fn bailian_model(api_key: &str) -> String {
    std::env::var("E2E_BAILIAN_MODEL").unwrap_or_else(|_| {
        if api_key.starts_with("sk-sp-") {
            "qwen3-coder-plus".to_string()
        } else {
            "qwen-turbo-latest".to_string()
        }
    })
}

/// Build an HTTP client with a generous timeout for E2E tests.
pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap()
}

/// Collect SSE events from a streaming response body text.
/// Returns a Vec of (event_type, data) tuples.
pub fn parse_sse_events(body: &str) -> Vec<(Option<String>, String)> {
    let mut events = Vec::new();
    let mut current_event_type: Option<String> = None;
    let mut current_data = String::new();

    for line in body.lines() {
        if let Some(event) = line.strip_prefix("event: ") {
            current_event_type = Some(event.to_string());
        } else if let Some(data) = line.strip_prefix("data: ") {
            if !current_data.is_empty() {
                current_data.push('\n');
            }
            current_data.push_str(data);
        } else if line.is_empty() && !current_data.is_empty() {
            events.push((current_event_type.take(), current_data.clone()));
            current_data.clear();
        }
    }

    // Flush any remaining data
    if !current_data.is_empty() {
        events.push((current_event_type.take(), current_data));
    }

    events
}
