use crate::payload::PayloadConfig;
use arc_swap::ArcSwap;
use notify::{RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

// ─── Config ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct Config {
    // Server
    pub host: String,
    pub port: u16,
    pub tls: TlsConfig,

    // Client auth
    pub api_keys: Vec<String>,
    #[serde(skip)]
    pub api_keys_set: HashSet<String>,

    // Global proxy
    pub proxy_url: Option<String>,

    // Debug & logging
    pub debug: bool,
    pub logging_to_file: bool,
    pub log_dir: Option<String>,

    // Routing
    pub routing: RoutingConfig,
    pub request_retry: u32,
    pub max_retry_interval: u64,

    // Timeouts (seconds)
    pub connect_timeout: u64,
    pub request_timeout: u64,

    // Streaming
    pub streaming: StreamingConfig,

    // Request body size limit (MB)
    pub body_limit_mb: usize,

    // Retry
    pub retry: RetryConfig,

    // Payload manipulation
    pub payload: PayloadConfig,

    // Upstream response headers to forward to clients
    pub passthrough_headers: Vec<String>,

    // Claude header defaults (injected when cloaking is active)
    pub claude_header_defaults: HashMap<String, String>,

    // Reject requests without model prefix when true
    pub force_model_prefix: bool,

    // Non-stream keepalive interval in seconds (0 = disabled).
    // When enabled, sends periodic whitespace to prevent intermediate proxy timeouts.
    pub non_stream_keepalive_secs: u64,

    // Dashboard
    pub dashboard: DashboardConfig,

    // Daemon
    pub daemon: DaemonConfig,

    // Provider credentials
    pub claude_api_key: Vec<ProviderKeyEntry>,
    pub openai_api_key: Vec<ProviderKeyEntry>,
    pub gemini_api_key: Vec<ProviderKeyEntry>,
    pub openai_compatibility: Vec<ProviderKeyEntry>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8317,
            tls: TlsConfig::default(),
            api_keys: Vec::new(),
            api_keys_set: HashSet::new(),
            proxy_url: None,
            debug: false,
            logging_to_file: false,
            log_dir: None,
            routing: RoutingConfig::default(),
            request_retry: 3,
            max_retry_interval: 30,
            connect_timeout: 30,
            request_timeout: 300,
            streaming: StreamingConfig::default(),
            body_limit_mb: 10,
            retry: RetryConfig::default(),
            payload: PayloadConfig::default(),
            passthrough_headers: Vec::new(),
            claude_header_defaults: HashMap::new(),
            force_model_prefix: false,
            non_stream_keepalive_secs: 0,
            dashboard: DashboardConfig::default(),
            daemon: DaemonConfig::default(),
            claude_api_key: Vec::new(),
            openai_api_key: Vec::new(),
            gemini_api_key: Vec::new(),
            openai_compatibility: Vec::new(),
        }
    }
}

impl Config {
    /// Load config from a YAML file, sanitize, and validate.
    pub fn load(path: &str) -> Result<Self, anyhow::Error> {
        let contents = std::fs::read_to_string(path)?;
        let mut config: Config = serde_yml::from_str(&contents)?;
        config.sanitize();
        config.validate()?;
        Ok(config)
    }

    /// Validate configuration.
    fn validate(&self) -> Result<(), anyhow::Error> {
        if self.tls.enable {
            anyhow::ensure!(self.tls.cert.is_some(), "TLS enabled but cert path missing");
            anyhow::ensure!(self.tls.key.is_some(), "TLS enabled but key path missing");
        }
        for entry in self.all_provider_keys() {
            if let Some(ref proxy) = entry.proxy_url {
                crate::proxy::validate_proxy_url(proxy)?;
            }
        }
        if let Some(ref proxy) = self.proxy_url {
            crate::proxy::validate_proxy_url(proxy)?;
        }
        Ok(())
    }

    /// Sanitize and normalize configuration.
    fn sanitize(&mut self) {
        sanitize_entries(&mut self.claude_api_key);
        sanitize_entries(&mut self.openai_api_key);
        sanitize_entries(&mut self.gemini_api_key);
        sanitize_entries(&mut self.openai_compatibility);

        // Build HashSet for O(1) API key lookups
        self.api_keys_set = self.api_keys.iter().cloned().collect();
    }

    /// Returns an iterator over all provider key entries.
    pub fn all_provider_keys(&self) -> impl Iterator<Item = &ProviderKeyEntry> {
        self.claude_api_key
            .iter()
            .chain(self.openai_api_key.iter())
            .chain(self.gemini_api_key.iter())
            .chain(self.openai_compatibility.iter())
    }
}

/// Remove entries with empty api_key, deduplicate, normalize base_url.
fn sanitize_entries(entries: &mut Vec<ProviderKeyEntry>) {
    // Remove entries with empty API keys
    entries.retain(|e| !e.api_key.is_empty());

    // Deduplicate by api_key
    let mut seen = HashSet::new();
    entries.retain(|e| seen.insert(e.api_key.clone()));

    // Normalize entries
    for entry in entries.iter_mut() {
        // Strip trailing slash from base_url
        if let Some(ref mut url) = entry.base_url {
            while url.ends_with('/') {
                url.pop();
            }
        }
        // Normalize header keys to lowercase
        let headers: HashMap<String, String> = entry
            .headers
            .drain()
            .map(|(k, v)| (k.to_lowercase(), v))
            .collect();
        entry.headers = headers;
    }
}

// ─── Dashboard config ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct DashboardConfig {
    /// Enable the dashboard admin API.
    pub enabled: bool,
    /// Admin username.
    pub username: String,
    /// bcrypt-hashed password (e.g. "$2b$12$...").
    pub password_hash: String,
    /// JWT signing secret. Falls back to env `DASHBOARD_JWT_SECRET`.
    pub jwt_secret: Option<String>,
    /// JWT token TTL in seconds.
    pub jwt_ttl_secs: u64,
    /// Request log ring buffer capacity.
    pub request_log_capacity: usize,
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            username: "admin".to_string(),
            password_hash: String::new(),
            jwt_secret: None,
            jwt_ttl_secs: 3600,
            request_log_capacity: 10_000,
        }
    }
}

impl DashboardConfig {
    /// Resolve the JWT secret from config or env.
    pub fn resolve_jwt_secret(&self) -> Option<String> {
        self.jwt_secret
            .clone()
            .or_else(|| std::env::var("DASHBOARD_JWT_SECRET").ok())
    }
}

// ─── Daemon config ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct DaemonConfig {
    /// Path to the PID file.
    pub pid_file: String,
    /// Graceful shutdown timeout in seconds.
    pub shutdown_timeout: u64,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            pid_file: "./ai-proxy.pid".to_string(),
            shutdown_timeout: 30,
        }
    }
}

// ─── Sub-configs ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case", default)]
pub struct TlsConfig {
    pub enable: bool,
    pub cert: Option<String>,
    pub key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct RoutingConfig {
    pub strategy: RoutingStrategy,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            strategy: RoutingStrategy::RoundRobin,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RoutingStrategy {
    RoundRobin,
    FillFirst,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct StreamingConfig {
    pub keepalive_seconds: u64,
    /// Max retries before first byte is sent to client (streaming bootstrap retry).
    pub bootstrap_retries: u32,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            keepalive_seconds: 15,
            bootstrap_retries: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub max_backoff_secs: u64,
    pub cooldown_429_secs: u64,
    pub cooldown_5xx_secs: u64,
    pub cooldown_network_secs: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            max_backoff_secs: 30,
            cooldown_429_secs: 60,
            cooldown_5xx_secs: 15,
            cooldown_network_secs: 10,
        }
    }
}

// ─── Provider key entry ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ModelMapping {
    /// Original model name from the provider.
    pub id: String,
    /// Alias to expose through the proxy.
    #[serde(default)]
    pub alias: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProviderKeyEntry {
    pub api_key: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub proxy_url: Option<String>,
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default)]
    pub models: Vec<ModelMapping>,
    #[serde(default)]
    pub excluded_models: Vec<String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub disabled: bool,
    /// Human-readable name for this key entry (used for logging/identification).
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub cloak: crate::cloak::CloakConfig,
    /// Wire API format for OpenAI-compatible providers.
    #[serde(default)]
    pub wire_api: crate::provider::WireApi,
}

// ─── Config Watcher ────────────────────────────────────────────────────────

pub struct ConfigWatcher {
    _watcher: notify::RecommendedWatcher,
}

impl ConfigWatcher {
    /// Start watching a config file. On changes (debounced 150ms, SHA256 dedup),
    /// reload the config and atomically swap it in via ArcSwap.
    pub fn start(
        path: String,
        config: Arc<ArcSwap<Config>>,
        on_reload: impl Fn(&Config) + Send + Sync + 'static,
    ) -> Result<Self, anyhow::Error> {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(16);

        let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, _>| {
            if let Ok(event) = res
                && (event.kind.is_modify() || event.kind.is_create())
            {
                let _ = tx.blocking_send(());
            }
        })?;
        watcher.watch(Path::new(&path), RecursiveMode::NonRecursive)?;

        let path_clone = path.clone();
        tokio::spawn(async move {
            let mut last_hash: Option<[u8; 32]> = None;
            let mut debounce: Option<tokio::time::Instant> = None;

            loop {
                tokio::select! {
                    Some(()) = rx.recv() => {
                        debounce = Some(tokio::time::Instant::now() + Duration::from_millis(150));
                    }
                    _ = async {
                        match debounce {
                            Some(deadline) => tokio::time::sleep_until(deadline).await,
                            None => std::future::pending::<()>().await,
                        }
                    } => {
                        debounce = None;
                        match std::fs::read(&path_clone) {
                            Ok(contents) => {
                                let hash: [u8; 32] = sha2::Sha256::digest(&contents).into();
                                if last_hash.as_ref() == Some(&hash) {
                                    continue;
                                }
                                last_hash = Some(hash);

                                match Config::load(&path_clone) {
                                    Ok(new_cfg) => {
                                        tracing::info!("Configuration reloaded successfully");
                                        on_reload(&new_cfg);
                                        config.store(Arc::new(new_cfg));
                                    }
                                    Err(e) => {
                                        tracing::error!("Config reload failed: {e}");
                                    }
                                }
                            }
                            Err(e) => tracing::error!("Config file read failed: {e}"),
                        }
                    }
                }
            }
        });

        Ok(Self { _watcher: watcher })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = Config::default();
        assert_eq!(cfg.host, "0.0.0.0");
        assert_eq!(cfg.port, 8317);
        assert!(!cfg.tls.enable);
        assert_eq!(cfg.request_retry, 3);
        assert_eq!(cfg.max_retry_interval, 30);
        assert_eq!(cfg.connect_timeout, 30);
        assert_eq!(cfg.request_timeout, 300);
        assert_eq!(cfg.streaming.keepalive_seconds, 15);
        assert_eq!(cfg.body_limit_mb, 10);
        assert_eq!(cfg.retry.max_retries, 3);
        assert_eq!(cfg.retry.max_backoff_secs, 30);
        assert_eq!(cfg.retry.cooldown_429_secs, 60);
        assert_eq!(cfg.retry.cooldown_5xx_secs, 15);
        assert_eq!(cfg.retry.cooldown_network_secs, 10);
    }

    #[test]
    fn test_sanitize_entries() {
        let mut entries = vec![
            ProviderKeyEntry {
                api_key: "key1".into(),
                base_url: Some("https://api.example.com/".into()),
                proxy_url: None,
                prefix: None,
                models: vec![],
                excluded_models: vec![],
                headers: HashMap::from([("X-Custom".into(), "val".into())]),
                disabled: false,
                name: None,
                cloak: Default::default(),
                wire_api: crate::provider::WireApi::default(),
            },
            ProviderKeyEntry {
                api_key: "".into(),
                base_url: None,
                proxy_url: None,
                prefix: None,
                models: vec![],
                excluded_models: vec![],
                headers: HashMap::new(),
                disabled: false,
                name: None,
                cloak: Default::default(),
                wire_api: crate::provider::WireApi::default(),
            },
            ProviderKeyEntry {
                api_key: "key1".into(), // duplicate
                base_url: None,
                proxy_url: None,
                prefix: None,
                models: vec![],
                excluded_models: vec![],
                headers: HashMap::new(),
                disabled: false,
                name: None,
                cloak: Default::default(),
                wire_api: crate::provider::WireApi::default(),
            },
        ];
        sanitize_entries(&mut entries);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].base_url.as_deref(),
            Some("https://api.example.com")
        );
        assert!(entries[0].headers.contains_key("x-custom"));
    }

    #[test]
    fn test_yaml_deserialization() {
        let yaml = r#"
host: "127.0.0.1"
port: 9000
api-keys:
  - "test-key"
routing:
  strategy: fill-first
claude-api-key:
  - api-key: "sk-ant-xxx"
    base-url: "https://api.anthropic.com"
    models:
      - id: "claude-sonnet-4-20250514"
        alias: "sonnet"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 9000);
        assert_eq!(config.api_keys, vec!["test-key"]);
        assert_eq!(config.routing.strategy, RoutingStrategy::FillFirst);
        assert_eq!(config.claude_api_key.len(), 1);
        assert_eq!(config.claude_api_key[0].models.len(), 1);
    }

    #[test]
    fn test_daemon_config_defaults() {
        let dc = DaemonConfig::default();
        assert_eq!(dc.pid_file, "./ai-proxy.pid");
        assert_eq!(dc.shutdown_timeout, 30);
    }

    #[test]
    fn test_daemon_config_yaml_round_trip() {
        let yaml = r#"
daemon:
  pid-file: "/run/ai-proxy.pid"
  shutdown-timeout: 60
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert_eq!(config.daemon.pid_file, "/run/ai-proxy.pid");
        assert_eq!(config.daemon.shutdown_timeout, 60);

        // Round-trip
        let serialized = serde_yml::to_string(&config).unwrap();
        let config2: Config = serde_yml::from_str(&serialized).unwrap();
        assert_eq!(config2.daemon.pid_file, "/run/ai-proxy.pid");
        assert_eq!(config2.daemon.shutdown_timeout, 60);
    }
}
