use crate::auth_key::{AuthKeyEntry, AuthKeyStore};
use crate::cache::CacheConfig;
use crate::circuit_breaker::CircuitBreakerConfig;
use crate::file_audit::FileAuditConfig;
use crate::payload::PayloadConfig;
use crate::request_record::LogDetailLevel;
pub use crate::routing::config::RoutingConfig;
use crate::thinking_cache::ThinkingCacheConfig;
use arc_swap::ArcSwap;
use notify::{RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::HashMap;
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

    // Client auth — structured auth keys
    pub auth_keys: Vec<AuthKeyEntry>,
    #[serde(skip)]
    pub auth_key_store: AuthKeyStore,

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
    pub non_stream_keepalive_secs: u64,

    // Cost tracking: custom model price overrides (USD per 1M tokens).
    pub model_prices: std::collections::HashMap<String, crate::cost::ModelPrice>,

    // Rate limiting
    pub rate_limit: RateLimitConfig,

    // Circuit breaker
    pub circuit_breaker: CircuitBreakerConfig,

    // Response cache
    pub cache: CacheConfig,

    // Log store
    #[serde(alias = "audit")]
    pub log_store: LogStoreConfig,

    // Dashboard
    pub dashboard: DashboardConfig,

    // Daemon
    pub daemon: DaemonConfig,

    // Thinking signature cache
    pub thinking_cache: ThinkingCacheConfig,

    // Quota-aware credential cooldown duration in seconds (default: 60).
    pub quota_cooldown_default_secs: u64,

    // Provider credentials (unified)
    #[serde(default)]
    pub providers: Vec<ProviderKeyEntry>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8317,
            tls: TlsConfig::default(),
            auth_keys: Vec::new(),
            auth_key_store: AuthKeyStore::default(),
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
            model_prices: HashMap::new(),
            rate_limit: RateLimitConfig::default(),
            circuit_breaker: CircuitBreakerConfig::default(),
            cache: CacheConfig::default(),
            log_store: LogStoreConfig::default(),
            dashboard: DashboardConfig::default(),
            daemon: DaemonConfig::default(),
            thinking_cache: ThinkingCacheConfig::default(),
            quota_cooldown_default_secs: 60,
            providers: Vec::new(),
        }
    }
}

impl Config {
    /// Load config from a YAML file, sanitize, and validate.
    pub fn load(path: &str) -> Result<Self, anyhow::Error> {
        let contents = std::fs::read_to_string(path)?;
        Self::load_from_str(&contents)
    }

    /// Parse config from a string (avoids re-reading the file).
    pub fn load_from_str(contents: &str) -> Result<Self, anyhow::Error> {
        let mut config: Config = serde_yaml_ng::from_str(contents)?;
        config.sanitize()?;
        config.validate()?;
        Ok(config)
    }

    /// Deserialize config from a YAML string with sanitization (but no validation).
    /// Used by dashboard config editing where the caller may mutate before writing back.
    pub fn from_yaml(yaml: &str) -> Result<Self, anyhow::Error> {
        let mut config: Config = serde_yaml_ng::from_str(yaml)?;
        config.sanitize()?;
        Ok(config)
    }

    /// Deserialize config from a YAML string **without** secret resolution.
    /// Entry normalization (dedup, URL cleanup) is still applied.
    /// Used by dashboard config writes to preserve `env://` and `file://` references.
    pub fn from_yaml_raw(yaml: &str) -> Result<Self, anyhow::Error> {
        let mut config: Config = serde_yaml_ng::from_str(yaml)?;
        config.normalize();
        Ok(config)
    }

    /// Serialize config to a YAML string.
    pub fn to_yaml(&self) -> Result<String, anyhow::Error> {
        Ok(serde_yaml_ng::to_string(self)?)
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
        // Provider name uniqueness
        let mut seen_names = std::collections::HashSet::new();
        for entry in &self.providers {
            anyhow::ensure!(!entry.name.is_empty(), "provider name must not be empty");
            anyhow::ensure!(
                seen_names.insert(&entry.name),
                "duplicate provider name: {}",
                entry.name
            );
            let mut seen_profile_ids = std::collections::HashSet::new();
            for profile in &entry.auth_profiles {
                profile
                    .validate()
                    .map_err(|e| anyhow::anyhow!("provider '{}': {e}", entry.name))?;
                profile
                    .validate_for_provider(entry.format, entry.base_url.as_deref())
                    .map_err(|e| anyhow::anyhow!("provider '{}': {e}", entry.name))?;
                anyhow::ensure!(
                    seen_profile_ids.insert(&profile.id),
                    "duplicate auth profile id '{}' in provider '{}'",
                    profile.id,
                    entry.name
                );
            }
        }
        self.routing
            .validate()
            .map_err(|e| anyhow::anyhow!("routing: {e}"))?;
        Ok(())
    }

    /// Normalize entries without resolving secrets.
    /// Safe for the persistence path (dashboard config writes).
    fn normalize(&mut self) {
        sanitize_entries(&mut self.providers);
        self.migrate_legacy_presentation();
    }

    /// Migrate legacy `cloak` + `headers` config into `upstream-presentation`.
    fn migrate_legacy_presentation(&mut self) {
        use crate::cloak::CloakMode;
        use crate::presentation::{ActivationMode, ProfileKind, UpstreamPresentationConfig};

        for entry in &mut self.providers {
            let has_explicit_presentation = entry.upstream_presentation.profile
                != ProfileKind::Native
                || !entry.upstream_presentation.custom_headers.is_empty();

            if has_explicit_presentation {
                // upstream-presentation is explicitly set; ignore legacy fields
                continue;
            }

            if entry.cloak.mode != CloakMode::Never {
                // Migrate cloak → claude-code profile
                entry.upstream_presentation = UpstreamPresentationConfig {
                    profile: ProfileKind::ClaudeCode,
                    mode: match entry.cloak.mode {
                        CloakMode::Always => ActivationMode::Always,
                        CloakMode::Auto => ActivationMode::Auto,
                        CloakMode::Never => ActivationMode::Always,
                    },
                    strict_mode: entry.cloak.strict_mode,
                    sensitive_words: entry.cloak.sensitive_words.clone(),
                    cache_user_id: entry.cloak.cache_user_id,
                    custom_headers: entry.headers.clone(),
                };
                // Clear legacy fields to avoid double-application
                entry.cloak = crate::cloak::CloakConfig::default();
                entry.headers.clear();
            } else if !entry.headers.is_empty() {
                // Migrate standalone headers → native profile with custom-headers
                entry.upstream_presentation.custom_headers = entry.headers.clone();
                entry.headers.clear();
            }
        }
    }

    /// Sanitize and normalize configuration, including secret resolution.
    /// Returns an error if any `env://` or `file://` secret reference cannot be resolved.
    fn sanitize(&mut self) -> Result<(), anyhow::Error> {
        self.normalize();

        // Resolve secrets in provider API keys
        resolve_provider_secrets(&mut self.providers)?;

        // Resolve secrets in auth keys
        for entry in &mut self.auth_keys {
            entry.key = crate::secret::resolve(&entry.key).map_err(|e| {
                anyhow::anyhow!(
                    "auth-key '{}': {e}",
                    entry.name.as_deref().unwrap_or("unnamed")
                )
            })?;
        }

        // Resolve secrets in dashboard config
        self.dashboard.password_hash = crate::secret::resolve(&self.dashboard.password_hash)
            .map_err(|e| anyhow::anyhow!("dashboard.password-hash: {e}"))?;
        if let Some(ref secret) = self.dashboard.jwt_secret {
            self.dashboard.jwt_secret = Some(
                crate::secret::resolve(secret)
                    .map_err(|e| anyhow::anyhow!("dashboard.jwt-secret: {e}"))?,
            );
        }

        // Build AuthKeyStore for O(1) auth key lookups
        self.auth_key_store = AuthKeyStore::new(self.auth_keys.clone());
        Ok(())
    }

    /// Returns an iterator over all provider key entries.
    pub fn all_provider_keys(&self) -> impl Iterator<Item = &ProviderKeyEntry> {
        self.providers.iter()
    }
}

/// Resolve env:// and file:// secrets in provider API keys,
/// and resolve credential_source if present.
fn resolve_provider_secrets(entries: &mut [ProviderKeyEntry]) -> Result<(), anyhow::Error> {
    for entry in entries.iter_mut() {
        // If credential_source is set, resolve it and use as api_key
        if let Some(ref source) = entry.credential_source {
            match source.resolve() {
                Ok(key) => {
                    entry.api_key = key;
                }
                Err(e) => {
                    tracing::warn!(
                        "provider '{}': failed to resolve credential source: {e}",
                        entry.name
                    );
                    // Fall through to resolve api_key normally if it's non-empty
                }
            }
        }

        if !entry.api_key.is_empty() {
            entry.api_key = crate::secret::resolve(&entry.api_key)
                .map_err(|e| anyhow::anyhow!("provider '{}': {e}", entry.name))?;
        }

        for profile in &mut entry.auth_profiles {
            profile.resolve_secrets().map_err(|e| {
                anyhow::anyhow!(
                    "provider '{}' auth-profile '{}': {e}",
                    entry.name,
                    profile.id
                )
            })?;
        }
    }
    Ok(())
}

/// Normalize provider entries for runtime and persistence.
fn sanitize_entries(entries: &mut [ProviderKeyEntry]) {
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
        for profile in &mut entry.auth_profiles {
            profile.normalize();
        }
    }
}

// ─── Rate limit config ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct RateLimitConfig {
    /// Enable rate limiting.
    pub enabled: bool,
    /// Global requests per minute limit (0 = unlimited).
    pub global_rpm: u32,
    /// Per-API-key requests per minute limit (0 = unlimited).
    pub per_key_rpm: u32,
    /// Global tokens per minute limit (0 = unlimited).
    pub global_tpm: u64,
    /// Per-API-key tokens per minute limit (0 = unlimited).
    pub per_key_tpm: u64,
    /// Per-API-key cost per day in USD (0.0 = unlimited).
    pub per_key_cost_per_day_usd: f64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            global_rpm: 0,
            per_key_rpm: 0,
            global_tpm: 0,
            per_key_tpm: 0,
            per_key_cost_per_day_usd: 0.0,
        }
    }
}

// ─── Log store config ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct LogStoreConfig {
    /// Log store backend type.
    pub backend: LogStoreBackend,
    /// Ring buffer capacity (memory backend).
    pub capacity: usize,
    /// How much request/response body content to capture.
    pub detail_level: LogDetailLevel,
    /// Maximum bytes of body content per field. 0 = unlimited.
    pub max_body_bytes: usize,
    /// Optional file audit (JSONL persistence).
    pub file_audit: FileAuditConfig,
}

impl Default for LogStoreConfig {
    fn default() -> Self {
        Self {
            backend: LogStoreBackend::Memory,
            capacity: 1_000,
            detail_level: LogDetailLevel::Metadata,
            max_body_bytes: 1_048_576,
            file_audit: FileAuditConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum LogStoreBackend {
    #[default]
    Memory,
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
    /// Max login attempts per IP within the lockout window before lockout.
    pub max_login_attempts: u32,
    /// Lockout window in seconds. After max_login_attempts within this window, reject login.
    pub login_lockout_secs: u64,
    /// Restrict dashboard access to localhost only.
    pub localhost_only: bool,
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            username: "admin".to_string(),
            password_hash: String::new(),
            jwt_secret: None,
            jwt_ttl_secs: 3600,
            max_login_attempts: 5,
            login_lockout_secs: 300,
            localhost_only: false,
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
            pid_file: "./prism.pid".to_string(),
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
    /// Jitter factor for retry backoff (0.0 = no jitter, 1.0 = full jitter).
    pub jitter_factor: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            max_backoff_secs: 30,
            cooldown_429_secs: 60,
            cooldown_5xx_secs: 15,
            cooldown_network_secs: 10,
            jitter_factor: 1.0,
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
    /// Unique provider name (used as identity for routing).
    pub name: String,
    /// Wire protocol format.
    pub format: crate::provider::Format,
    #[serde(default)]
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
    #[serde(default)]
    pub cloak: crate::cloak::CloakConfig,
    /// Wire API format for OpenAI-protocol providers (Chat or Responses).
    #[serde(default)]
    pub wire_api: crate::provider::WireApi,
    /// Weight for weighted round-robin routing (default: 1, range 1-100).
    #[serde(default = "default_weight")]
    pub weight: u32,
    /// Region identifier for geo-aware routing.
    #[serde(default)]
    pub region: Option<String>,
    /// Optional credential source (defaults to static API key from `api_key` field).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_source: Option<crate::credential_source::CredentialSource>,
    /// Nested auth profiles for this logical provider family.
    #[serde(default)]
    pub auth_profiles: Vec<crate::auth_profile::AuthProfileEntry>,
    /// Upstream presentation configuration (profile-first identity for upstream requests).
    #[serde(default)]
    pub upstream_presentation: crate::presentation::UpstreamPresentationConfig,
    /// Whether this is a Vertex AI credential (uses Bearer auth + Vertex URL pattern).
    #[serde(default)]
    pub vertex: bool,
    /// Vertex AI project ID (required when `vertex: true`).
    #[serde(default)]
    pub vertex_project: Option<String>,
    /// Vertex AI location (required when `vertex: true`, e.g. "us-central1").
    #[serde(default)]
    pub vertex_location: Option<String>,
}

impl ProviderKeyEntry {
    /// Expand auth profiles. If none are configured, legacy provider-level auth
    /// fields are exposed as a single implicit profile using the provider name.
    pub fn expanded_auth_profiles(&self) -> Vec<crate::auth_profile::AuthProfileEntry> {
        if !self.auth_profiles.is_empty() {
            return self.auth_profiles.clone();
        }

        if self.api_key.is_empty() {
            return Vec::new();
        }

        vec![crate::auth_profile::AuthProfileEntry {
            id: self.name.clone(),
            mode: crate::auth_profile::AuthMode::ApiKey,
            header: crate::auth_profile::AuthHeaderKind::Auto,
            secret: (!self.api_key.is_empty()).then(|| self.api_key.clone()),
            headers: self.headers.clone(),
            disabled: self.disabled,
            weight: self.weight.max(1),
            region: self.region.clone(),
            prefix: self.prefix.clone(),
            upstream_presentation: self.upstream_presentation.clone(),
            ..Default::default()
        }]
    }
}

fn default_weight() -> u32 {
    1
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
                        match std::fs::read_to_string(&path_clone) {
                            Ok(contents) => {
                                let hash: [u8; 32] = sha2::Sha256::digest(contents.as_bytes()).into();
                                if last_hash.as_ref() == Some(&hash) {
                                    continue;
                                }
                                last_hash = Some(hash);

                                match Config::load_from_str(&contents) {
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
        assert!(!cfg.cache.enabled);
        assert!(!cfg.log_store.file_audit.enabled);
        assert!(cfg.circuit_breaker.enabled);
    }

    fn make_test_entry(name: &str, api_key: &str) -> ProviderKeyEntry {
        ProviderKeyEntry {
            name: name.to_string(),
            format: crate::provider::Format::OpenAI,
            api_key: api_key.into(),
            base_url: None,
            proxy_url: None,
            prefix: None,
            models: vec![],
            excluded_models: vec![],
            headers: HashMap::new(),
            disabled: false,
            cloak: Default::default(),
            upstream_presentation: Default::default(),
            wire_api: crate::provider::WireApi::default(),
            weight: 1,
            region: None,
            credential_source: None,
            auth_profiles: vec![],
            vertex: false,
            vertex_project: None,
            vertex_location: None,
        }
    }

    #[test]
    fn test_sanitize_entries() {
        let mut e1 = make_test_entry("p1", "key1");
        e1.base_url = Some("https://api.example.com/".into());
        e1.headers = HashMap::from([("X-Custom".into(), "val".into())]);
        let e2 = make_test_entry("p2", "");
        let mut e3 = make_test_entry("p3", "key1");
        e3.format = crate::provider::Format::Claude;
        let mut entries = vec![e1, e2, e3];
        sanitize_entries(&mut entries);
        assert_eq!(entries.len(), 3);
        assert_eq!(
            entries[0].base_url.as_deref(),
            Some("https://api.example.com")
        );
        assert!(entries[0].headers.contains_key("x-custom"));
        assert_eq!(entries[1].name, "p2");
        assert!(entries[1].expanded_auth_profiles().is_empty());
        assert_eq!(entries[2].name, "p3");
        assert_eq!(entries[2].format, crate::provider::Format::Claude);
    }

    #[test]
    fn test_yaml_deserialization() {
        let yaml = r#"
host: "127.0.0.1"
port: 9000
auth-keys:
  - key: "test-key"
    name: "Test"
    tenant-id: "t1"
    allowed-models: ["claude-*"]
routing:
  default-profile: balanced
providers:
  - name: claude
    format: claude
    api-key: "sk-ant-xxx"
    base-url: "https://api.anthropic.com"
    models:
      - id: "claude-sonnet-4-20250514"
        alias: "sonnet"
"#;
        let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 9000);
        assert_eq!(config.auth_keys.len(), 1);
        assert_eq!(config.auth_keys[0].key, "test-key");
        assert_eq!(config.auth_keys[0].tenant_id.as_deref(), Some("t1"));
        assert_eq!(config.routing.default_profile, "balanced");
        assert_eq!(config.providers.len(), 1);
        assert_eq!(config.providers[0].models.len(), 1);
        assert_eq!(config.providers[0].format, crate::provider::Format::Claude);
        assert!(config.providers[0].auth_profiles.is_empty());
    }

    #[test]
    fn test_daemon_config_defaults() {
        let dc = DaemonConfig::default();
        assert_eq!(dc.pid_file, "./prism.pid");
        assert_eq!(dc.shutdown_timeout, 30);
    }

    #[test]
    fn test_daemon_config_yaml_round_trip() {
        let yaml = r#"
daemon:
  pid-file: "/run/prism.pid"
  shutdown-timeout: 60
"#;
        let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(config.daemon.pid_file, "/run/prism.pid");
        assert_eq!(config.daemon.shutdown_timeout, 60);

        // Round-trip
        let serialized = serde_yaml_ng::to_string(&config).unwrap();
        let config2: Config = serde_yaml_ng::from_str(&serialized).unwrap();
        assert_eq!(config2.daemon.pid_file, "/run/prism.pid");
        assert_eq!(config2.daemon.shutdown_timeout, 60);
    }

    #[test]
    fn test_rate_limit_config_new_fields() {
        let yaml = r#"
rate-limit:
  enabled: true
  global-rpm: 100
  per-key-rpm: 50
  global-tpm: 1000000
  per-key-tpm: 500000
  per-key-cost-per-day-usd: 10.0
"#;
        let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
        assert!(config.rate_limit.enabled);
        assert_eq!(config.rate_limit.global_tpm, 1_000_000);
        assert_eq!(config.rate_limit.per_key_tpm, 500_000);
        assert_eq!(config.rate_limit.per_key_cost_per_day_usd, 10.0);
    }

    #[test]
    fn test_routing_config_defaults_in_config() {
        let config = Config::default();
        assert_eq!(config.routing.default_profile, "balanced");
        assert_eq!(config.routing.profiles.len(), 4);
    }

    #[test]
    fn test_auth_key_allowed_credentials() {
        let yaml = r#"
auth-keys:
  - key: "test-key"
    allowed-credentials: ["my-claude-*", "shared-*"]
"#;
        let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(config.auth_keys[0].allowed_credentials.len(), 2);
        assert_eq!(config.auth_keys[0].allowed_credentials[0], "my-claude-*");
    }

    #[test]
    fn test_env_secret_resolution_auth_keys() {
        unsafe { std::env::set_var("TEST_PRISM_AUTH_KEY", "sk-resolved-key") };
        let yaml = r#"
auth-keys:
  - key: "env://TEST_PRISM_AUTH_KEY"
    name: "env-test"
"#;
        let config = Config::load_from_str(yaml).unwrap();
        assert_eq!(config.auth_keys[0].key, "sk-resolved-key");
        assert_eq!(
            config
                .auth_key_store
                .lookup("sk-resolved-key")
                .unwrap()
                .name
                .as_deref(),
            Some("env-test")
        );
        unsafe { std::env::remove_var("TEST_PRISM_AUTH_KEY") };
    }

    #[test]
    fn test_env_secret_resolution_dashboard() {
        unsafe {
            std::env::set_var(
                "TEST_PRISM_DASH_HASH",
                "$2y$12$abcdefghijklmnopqrstuuABCDEFGHIJKLMNOPQRSTUVWXYZ012345",
            )
        };
        unsafe { std::env::set_var("TEST_PRISM_JWT_SECRET", "my-jwt-secret") };
        let yaml = r#"
dashboard:
  enabled: true
  password-hash: "env://TEST_PRISM_DASH_HASH"
  jwt-secret: "env://TEST_PRISM_JWT_SECRET"
"#;
        let config = Config::load_from_str(yaml).unwrap();
        assert_eq!(
            config.dashboard.password_hash,
            "$2y$12$abcdefghijklmnopqrstuuABCDEFGHIJKLMNOPQRSTUVWXYZ012345"
        );
        assert_eq!(
            config.dashboard.jwt_secret.as_deref(),
            Some("my-jwt-secret")
        );
        unsafe { std::env::remove_var("TEST_PRISM_DASH_HASH") };
        unsafe { std::env::remove_var("TEST_PRISM_JWT_SECRET") };
    }

    #[test]
    fn test_from_yaml_raw_preserves_secret_references() {
        unsafe { std::env::set_var("TEST_RAW_API_KEY", "resolved-secret") };

        let yaml = r#"
providers:
  - name: test-claude
    format: claude
    api-key: "env://TEST_RAW_API_KEY"
auth-keys:
  - key: "env://TEST_RAW_API_KEY"
    name: "test-auth"
dashboard:
  enabled: true
  password-hash: "env://TEST_RAW_API_KEY"
  jwt-secret: "env://TEST_RAW_API_KEY"
"#;
        // from_yaml_raw should NOT resolve secrets
        let raw = Config::from_yaml_raw(yaml).unwrap();
        assert_eq!(raw.providers[0].api_key, "env://TEST_RAW_API_KEY");
        assert_eq!(raw.auth_keys[0].key, "env://TEST_RAW_API_KEY");
        assert_eq!(raw.dashboard.password_hash, "env://TEST_RAW_API_KEY");
        assert_eq!(
            raw.dashboard.jwt_secret.as_deref(),
            Some("env://TEST_RAW_API_KEY")
        );

        // Round-trip: serialize and re-parse should still preserve references
        let serialized = raw.to_yaml().unwrap();
        let raw2 = Config::from_yaml_raw(&serialized).unwrap();
        assert_eq!(raw2.providers[0].api_key, "env://TEST_RAW_API_KEY");
        assert_eq!(raw2.auth_keys[0].key, "env://TEST_RAW_API_KEY");
        assert_eq!(raw2.dashboard.password_hash, "env://TEST_RAW_API_KEY");

        // from_yaml (with sanitize) SHOULD resolve secrets
        let resolved = Config::from_yaml(yaml).unwrap();
        assert_eq!(resolved.providers[0].api_key, "resolved-secret");
        assert_eq!(resolved.auth_keys[0].key, "resolved-secret");

        unsafe { std::env::remove_var("TEST_RAW_API_KEY") };
    }

    #[test]
    fn test_from_yaml_raw_preserves_file_secret_references() {
        let dir = tempfile::tempdir().unwrap();
        let secret_path = dir.path().join("api-key.txt");
        std::fs::write(&secret_path, "file-secret-value\n").unwrap();
        let file_ref = format!("file://{}", secret_path.display());

        let yaml = format!(
            r#"
providers:
  - name: file-test
    format: claude
    api-key: "{file_ref}"
"#
        );

        // from_yaml_raw preserves file:// references
        let raw = Config::from_yaml_raw(&yaml).unwrap();
        assert_eq!(raw.providers[0].api_key, file_ref);

        // from_yaml resolves them
        let resolved = Config::from_yaml(&yaml).unwrap();
        assert_eq!(resolved.providers[0].api_key, "file-secret-value");
    }

    #[test]
    fn test_nested_auth_profiles_yaml() {
        let yaml = r#"
providers:
  - name: anthropic
    format: claude
    auth-profiles:
      - id: billing
        mode: api-key
        secret: "sk-ant-api"
      - id: subscription
        mode: bearer-token
        secret: "token-123"
"#;
        let config = Config::from_yaml(yaml).unwrap();
        let provider = &config.providers[0];
        assert_eq!(provider.auth_profiles.len(), 2);
        assert_eq!(provider.auth_profiles[0].id, "billing");
        assert_eq!(
            provider.auth_profiles[1].mode,
            crate::auth_profile::AuthMode::BearerToken
        );
    }

    #[test]
    fn test_expanded_auth_profiles_legacy() {
        let entry = make_test_entry("legacy", "sk-legacy");
        let profiles = entry.expanded_auth_profiles();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].id, "legacy");
        assert_eq!(profiles[0].secret.as_deref(), Some("sk-legacy"));
    }

    #[test]
    fn test_model_rewrites_in_config() {
        let yaml = r#"
routing:
  model-resolution:
    rewrites:
      - pattern: "gpt-4"
        to: "gpt-4-turbo"
      - pattern: "claude-*"
        to: "claude-sonnet-4-20250514"
"#;
        let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(
            config.routing.resolve_model_rewrite("gpt-4"),
            Some("gpt-4-turbo")
        );
        assert_eq!(
            config.routing.resolve_model_rewrite("claude-3-opus"),
            Some("claude-sonnet-4-20250514")
        );
        assert_eq!(config.routing.resolve_model_rewrite("gemini-pro"), None);
    }

    #[test]
    fn test_legacy_cloak_migration() {
        let yaml = r#"
providers:
  - name: my-claude
    format: claude
    api-key: "sk-ant-xxx"
    cloak:
      mode: auto
      strict-mode: true
      sensitive-words: ["proxy", "prism"]
      cache-user-id: true
    headers:
      x-custom: "value"
"#;
        let config = Config::from_yaml(yaml).unwrap();
        let entry = &config.providers[0];
        // Legacy cloak should be migrated to upstream-presentation
        assert_eq!(
            entry.upstream_presentation.profile,
            crate::presentation::ProfileKind::ClaudeCode
        );
        assert_eq!(
            entry.upstream_presentation.mode,
            crate::presentation::ActivationMode::Auto
        );
        assert!(entry.upstream_presentation.strict_mode);
        assert_eq!(
            entry.upstream_presentation.sensitive_words,
            vec!["proxy", "prism"]
        );
        assert!(entry.upstream_presentation.cache_user_id);
        assert_eq!(
            entry
                .upstream_presentation
                .custom_headers
                .get("x-custom")
                .unwrap(),
            "value"
        );
        // Legacy fields should be cleared
        assert_eq!(entry.cloak.mode, crate::cloak::CloakMode::Never);
        assert!(entry.headers.is_empty());
    }

    #[test]
    fn test_legacy_headers_only_migration() {
        let yaml = r#"
providers:
  - name: my-provider
    format: claude
    api-key: "sk-xxx"
    headers:
      x-custom: "value"
"#;
        let config = Config::from_yaml(yaml).unwrap();
        let entry = &config.providers[0];
        assert_eq!(
            entry.upstream_presentation.profile,
            crate::presentation::ProfileKind::Native
        );
        assert_eq!(
            entry
                .upstream_presentation
                .custom_headers
                .get("x-custom")
                .unwrap(),
            "value"
        );
        assert!(entry.headers.is_empty());
    }

    #[test]
    fn test_explicit_presentation_skips_migration() {
        let yaml = r#"
providers:
  - name: my-claude
    format: claude
    api-key: "sk-ant-xxx"
    upstream-presentation:
      profile: claude-code
      mode: always
    cloak:
      mode: always
    headers:
      x-old: "ignored"
"#;
        let config = Config::from_yaml(yaml).unwrap();
        let entry = &config.providers[0];
        // upstream-presentation is explicitly set, so legacy fields are NOT migrated
        assert_eq!(
            entry.upstream_presentation.profile,
            crate::presentation::ProfileKind::ClaudeCode
        );
        // Legacy fields remain untouched (not cleared)
        assert_eq!(entry.cloak.mode, crate::cloak::CloakMode::Always);
    }

    #[test]
    fn test_upstream_presentation_yaml_round_trip() {
        let yaml = r#"
providers:
  - name: my-claude
    format: claude
    api-key: "sk-ant-xxx"
    upstream-presentation:
      profile: claude-code
      mode: auto
      strict-mode: true
      sensitive-words: ["proxy"]
      cache-user-id: true
      custom-headers:
        x-extra: "val"
"#;
        let config = Config::from_yaml(yaml).unwrap();
        let entry = &config.providers[0];
        assert_eq!(
            entry.upstream_presentation.profile,
            crate::presentation::ProfileKind::ClaudeCode
        );
        assert_eq!(
            entry.upstream_presentation.mode,
            crate::presentation::ActivationMode::Auto
        );
        assert!(entry.upstream_presentation.strict_mode);
        assert_eq!(entry.upstream_presentation.sensitive_words, vec!["proxy"]);
        assert!(entry.upstream_presentation.cache_user_id);
        assert_eq!(
            entry
                .upstream_presentation
                .custom_headers
                .get("x-extra")
                .unwrap(),
            "val"
        );
    }
}
