# Configuration Types Reference

All configuration types used for YAML config parsing and runtime settings.

**Source:** `crates/core/src/config.rs`, `crates/core/src/payload.rs`, `crates/core/src/cloak.rs`, `crates/core/src/auth_key.rs`, `crates/core/src/cache.rs`, `crates/core/src/audit.rs`, `crates/core/src/circuit_breaker.rs`, `crates/core/src/cost.rs`

---

## Config

The root configuration struct. Loaded from YAML via `Config::load()`. Uses `#[serde(rename_all = "kebab-case", default)]`.

```rust
pub struct Config {
    pub host: String,
    pub port: u16,
    pub tls: TlsConfig,
    pub auth_keys: Vec<AuthKeyEntry>,
    pub auth_key_store: AuthKeyStore,
    pub proxy_url: Option<String>,
    pub debug: bool,
    pub logging_to_file: bool,
    pub log_dir: Option<String>,
    pub routing: RoutingConfig,
    pub request_retry: u32,
    pub max_retry_interval: u64,
    pub connect_timeout: u64,
    pub request_timeout: u64,
    pub streaming: StreamingConfig,
    pub body_limit_mb: usize,
    pub retry: RetryConfig,
    pub payload: PayloadConfig,
    pub passthrough_headers: Vec<String>,
    pub claude_header_defaults: HashMap<String, String>,
    pub force_model_prefix: bool,
    pub non_stream_keepalive_secs: u64,
    pub model_prices: HashMap<String, ModelPrice>,
    pub rate_limit: RateLimitConfig,
    pub circuit_breaker: CircuitBreakerConfig,
    pub cache: CacheConfig,
    pub log_store: LogStoreConfig,
    pub dashboard: DashboardConfig,
    pub daemon: DaemonConfig,
    pub thinking_cache: ThinkingCacheConfig,
    pub quota_cooldown_default_secs: u64,
    pub providers: Vec<ProviderKeyEntry>,
}
```

### Field defaults

| Field | Type | Default | YAML key |
|-------|------|---------|----------|
| `host` | `String` | `"0.0.0.0"` | `host` |
| `port` | `u16` | `8317` | `port` |
| `tls` | `TlsConfig` | disabled | `tls` |
| `auth_keys` | `Vec<AuthKeyEntry>` | `[]` | `auth-keys` |
| `proxy_url` | `Option<String>` | `None` | `proxy-url` |
| `debug` | `bool` | `false` | `debug` |
| `logging_to_file` | `bool` | `false` | `logging-to-file` |
| `log_dir` | `Option<String>` | `None` | `log-dir` |
| `routing` | `RoutingConfig` | round-robin | `routing` |
| `request_retry` | `u32` | `3` | `request-retry` |
| `max_retry_interval` | `u64` | `30` | `max-retry-interval` |
| `connect_timeout` | `u64` | `30` | `connect-timeout` |
| `request_timeout` | `u64` | `300` | `request-timeout` |
| `streaming` | `StreamingConfig` | see below | `streaming` |
| `body_limit_mb` | `usize` | `10` | `body-limit-mb` |
| `retry` | `RetryConfig` | see below | `retry` |
| `payload` | `PayloadConfig` | empty | `payload` |
| `passthrough_headers` | `Vec<String>` | `[]` | `passthrough-headers` |
| `claude_header_defaults` | `HashMap<String, String>` | `{}` | `claude-header-defaults` |
| `force_model_prefix` | `bool` | `false` | `force-model-prefix` |
| `non_stream_keepalive_secs` | `u64` | `0` (disabled) | `non-stream-keepalive-secs` |
| `model_prices` | `HashMap<String, ModelPrice>` | `{}` | `model-prices` |
| `rate_limit` | `RateLimitConfig` | disabled | `rate-limit` |
| `circuit_breaker` | `CircuitBreakerConfig` | enabled | `circuit-breaker` |
| `cache` | `CacheConfig` | disabled | `cache` |
| `log_store` | `LogStoreConfig` | memory backend | `log-store` (`audit` accepted as alias) |
| `dashboard` | `DashboardConfig` | disabled | `dashboard` |
| `daemon` | `DaemonConfig` | see below | `daemon` |
| `thinking_cache` | `ThinkingCacheConfig` | disabled | `thinking-cache` |
| `quota_cooldown_default_secs` | `u64` | `60` | `quota-cooldown-default-secs` |
| `providers` | `Vec<ProviderKeyEntry>` | `[]` | `providers` |

### Key methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `load` | `fn load(path: &str) -> Result<Self, anyhow::Error>` | Reads YAML, deserializes, sanitizes, and validates. |
| `load_from_str` | `fn load_from_str(contents: &str) -> Result<Self, anyhow::Error>` | Parses YAML from an in-memory string and runs sanitize + validate. |
| `from_yaml_raw` | `fn from_yaml_raw(yaml: &str) -> Result<Self, anyhow::Error>` | Parses YAML without resolving `env://` or `file://` secrets, used by dashboard writeback flows. |
| `to_yaml` | `fn to_yaml(&self) -> Result<String, anyhow::Error>` | Serializes the current config back to YAML. |
| `all_provider_keys` | `fn all_provider_keys(&self) -> impl Iterator<Item = &ProviderKeyEntry>` | Iterates the unified `providers[]` array. |

### Sanitization (on load)

- Trailing slashes are stripped from `base_url`.
- Header keys are normalized to lowercase.
- Nested auth profiles are normalized the same way, including header keys and zero-weight correction.
- `auth_key_store` is built from `auth_keys` for O(1) lookups.
- Secret-bearing fields are resolved through the secret resolver when using the validated load path.

### Validation highlights

- Provider names must be unique within `providers[]`.
- Auth profile IDs must be unique within each provider.
- Provider and global proxy URLs are validated at load time.

---

## AuthKeyEntry

**Source:** `crates/core/src/auth_key.rs`

Per-client API key with access control, rate limits, budgets, and expiry.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct AuthKeyEntry {
    pub key: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub tenant_id: Option<String>,
    #[serde(default)]
    pub allowed_models: Vec<String>,
    #[serde(default)]
    pub rate_limit: Option<KeyRateLimitConfig>,
    #[serde(default)]
    pub budget: Option<BudgetConfig>,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}
```

| Field | Type | Default | YAML key | Description |
|-------|------|---------|----------|-------------|
| `key` | `String` | required | `key` | Client API key string (e.g., `"sk-proxy-abc123"`). |
| `name` | `Option<String>` | `None` | `name` | Human-readable label for this key. |
| `tenant_id` | `Option<String>` | `None` | `tenant-id` | Tenant identifier for multi-tenant tracking. |
| `allowed_models` | `Vec<String>` | `[]` | `allowed-models` | Glob patterns restricting model access. Empty = all models allowed. |
| `rate_limit` | `Option<KeyRateLimitConfig>` | `None` | `rate-limit` | Per-key rate limit overrides. |
| `budget` | `Option<BudgetConfig>` | `None` | `budget` | Cost budget configuration. |
| `expires_at` | `Option<DateTime<Utc>>` | `None` | `expires-at` | Key expiry time (ISO 8601). Requests after this time get `KeyExpired` error. |
| `metadata` | `HashMap<String, String>` | `{}` | `metadata` | Arbitrary key-value metadata. |

### YAML example

```yaml
auth-keys:
  - key: "sk-proxy-team-alpha"
    name: "Team Alpha"
    tenant-id: "alpha"
    allowed-models: ["claude-*", "gpt-4o*"]
    rate-limit:
      rpm: 100
      tpm: 500000
    budget:
      total-usd: 500.0
      period: monthly
    expires-at: "2026-12-31T23:59:59Z"
```

---

## KeyRateLimitConfig

**Source:** `crates/core/src/auth_key.rs`

Per-key rate limit overrides (all fields optional).

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct KeyRateLimitConfig {
    pub rpm: Option<u32>,
    pub tpm: Option<u64>,
    pub cost_per_day_usd: Option<f64>,
}
```

| Field | Type | Default | YAML key | Description |
|-------|------|---------|----------|-------------|
| `rpm` | `Option<u32>` | `None` | `rpm` | Requests per minute limit for this key. |
| `tpm` | `Option<u64>` | `None` | `tpm` | Tokens per minute limit for this key. |
| `cost_per_day_usd` | `Option<f64>` | `None` | `cost-per-day-usd` | Daily cost limit in USD for this key. |

---

## BudgetConfig

**Source:** `crates/core/src/auth_key.rs`

Cost budget configuration for a key.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BudgetConfig {
    pub total_usd: f64,
    pub period: BudgetPeriod,
}
```

| Field | Type | YAML key | Description |
|-------|------|----------|-------------|
| `total_usd` | `f64` | `total-usd` | Maximum spend in USD for the budget period. |
| `period` | `BudgetPeriod` | `period` | Reset period: `"daily"` or `"monthly"`. |

---

## TlsConfig

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case", default)]
pub struct TlsConfig {
    pub enable: bool,
    pub cert: Option<String>,
    pub key: Option<String>,
}
```

| Field | Type | Default | YAML key |
|-------|------|---------|----------|
| `enable` | `bool` | `false` | `enable` |
| `cert` | `Option<String>` | `None` | `cert` |
| `key` | `Option<String>` | `None` | `key` |

Validation: if `enable` is `true`, both `cert` and `key` must be set.

### YAML example

```yaml
tls:
  enable: true
  cert: /path/to/cert.pem
  key: /path/to/key.pem
```

---

## RoutingConfig

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct RoutingConfig {
    pub strategy: RoutingStrategy,
    pub fallback_enabled: bool,
    pub ewma_alpha: f64,
    pub default_region: Option<String>,
}
```

| Field | Type | Default | YAML key | Description |
|-------|------|---------|----------|-------------|
| `strategy` | `RoutingStrategy` | `RoundRobin` | `strategy` | Credential selection strategy. |
| `fallback_enabled` | `bool` | `true` | `fallback-enabled` | Allow fallback to other providers on failure. |
| `ewma_alpha` | `f64` | `0.3` | `ewma-alpha` | EWMA smoothing factor for latency-aware routing (0.0-1.0). |
| `default_region` | `Option<String>` | `None` | `default-region` | Default region for geo-aware routing when client region is unknown. |

### YAML example

```yaml
routing:
  strategy: latency-aware
  fallback-enabled: true
  ewma-alpha: 0.3
  default-region: us-east
```

---

## StreamingConfig

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct StreamingConfig {
    pub keepalive_seconds: u64,
    pub bootstrap_retries: u32,
}
```

| Field | Type | Default | YAML key | Description |
|-------|------|---------|----------|-------------|
| `keepalive_seconds` | `u64` | `15` | `keepalive-seconds` | SSE keepalive interval during streaming. |
| `bootstrap_retries` | `u32` | `1` | `bootstrap-retries` | Max retries before first byte is sent to client. |

### YAML example

```yaml
streaming:
  keepalive-seconds: 15
  bootstrap-retries: 1
```

---

## RetryConfig

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub max_backoff_secs: u64,
    pub cooldown_429_secs: u64,
    pub cooldown_5xx_secs: u64,
    pub cooldown_network_secs: u64,
    pub jitter_factor: f64,
}
```

| Field | Type | Default | YAML key | Description |
|-------|------|---------|----------|-------------|
| `max_retries` | `u32` | `3` | `max-retries` | Maximum retry attempts across all providers. |
| `max_backoff_secs` | `u64` | `30` | `max-backoff-secs` | Cap for exponential backoff with jitter. |
| `cooldown_429_secs` | `u64` | `60` | `cooldown-429-secs` | Cooldown duration for rate-limited (429) credentials. Overridden by `Retry-After` header when present. |
| `cooldown_5xx_secs` | `u64` | `15` | `cooldown-5xx-secs` | Cooldown duration for 5xx errors. Overridden by `Retry-After` header when present. |
| `cooldown_network_secs` | `u64` | `10` | `cooldown-network-secs` | Cooldown duration for network errors (timeout, connection failure). |
| `jitter_factor` | `f64` | `1.0` | `jitter-factor` | Jitter factor for retry backoff (0.0 = no jitter, 1.0 = full jitter). |

### YAML example

```yaml
retry:
  max-retries: 3
  max-backoff-secs: 30
  cooldown-429-secs: 60
  cooldown-5xx-secs: 15
  cooldown-network-secs: 10
  jitter-factor: 1.0
```

---

## RateLimitConfig

**Source:** `crates/core/src/config.rs`

Global rate limiting configuration. All limits are enforced in-memory.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct RateLimitConfig {
    pub enabled: bool,
    pub global_rpm: u32,
    pub per_key_rpm: u32,
    pub global_tpm: u64,
    pub per_key_tpm: u64,
    pub per_key_cost_per_day_usd: f64,
}
```

| Field | Type | Default | YAML key | Description |
|-------|------|---------|----------|-------------|
| `enabled` | `bool` | `false` | `enabled` | Enable rate limiting. |
| `global_rpm` | `u32` | `0` | `global-rpm` | Global requests per minute limit (0 = unlimited). |
| `per_key_rpm` | `u32` | `0` | `per-key-rpm` | Per-API-key requests per minute limit (0 = unlimited). |
| `global_tpm` | `u64` | `0` | `global-tpm` | Global tokens per minute limit (0 = unlimited). |
| `per_key_tpm` | `u64` | `0` | `per-key-tpm` | Per-API-key tokens per minute limit (0 = unlimited). |
| `per_key_cost_per_day_usd` | `f64` | `0.0` | `per-key-cost-per-day-usd` | Per-API-key cost per day in USD (0.0 = unlimited). |

### YAML example

```yaml
rate-limit:
  enabled: true
  global-rpm: 1000
  per-key-rpm: 60
  per-key-cost-per-day-usd: 50.0
```

---

## CircuitBreakerConfig

**Source:** `crates/core/src/circuit_breaker.rs`

Per-credential circuit breaker configuration. Uses a three-state model (Closed -> Open -> HalfOpen -> Closed).

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct CircuitBreakerConfig {
    pub enabled: bool,
    pub failure_threshold: u32,
    pub cooldown_secs: u64,
    pub half_open_max_probes: u32,
    pub rolling_window_secs: u64,
}
```

| Field | Type | Default | YAML key | Description |
|-------|------|---------|----------|-------------|
| `enabled` | `bool` | `true` | `enabled` | Enable circuit breaker. When disabled, `NoopCircuitBreaker` is used. |
| `failure_threshold` | `u32` | `5` | `failure-threshold` | Consecutive failures within the rolling window to trip the circuit. |
| `cooldown_secs` | `u64` | `30` | `cooldown-secs` | How long the circuit stays open before transitioning to half-open. |
| `half_open_max_probes` | `u32` | `1` | `half-open-max-probes` | Number of probe requests allowed in half-open state. |
| `rolling_window_secs` | `u64` | `60` | `rolling-window-secs` | Rolling window for failure counting. Failures older than this are reset. |

### YAML example

```yaml
circuit-breaker:
  enabled: true
  failure-threshold: 5
  cooldown-secs: 30
  half-open-max-probes: 1
  rolling-window-secs: 60
```

---

## CacheConfig

**Source:** `crates/core/src/cache.rs`

Response cache configuration. Only caches non-streaming requests with temperature=0.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct CacheConfig {
    pub enabled: bool,
    pub max_entries: u64,
    pub ttl_secs: u64,
}
```

| Field | Type | Default | YAML key | Description |
|-------|------|---------|----------|-------------|
| `enabled` | `bool` | `false` | `enabled` | Enable response caching. |
| `max_entries` | `u64` | `10_000` | `max-entries` | Maximum number of cached entries. |
| `ttl_secs` | `u64` | `3600` | `ttl-secs` | Time-to-live for cached entries in seconds. |

### YAML example

```yaml
cache:
  enabled: true
  max-entries: 10000
  ttl-secs: 3600
```

---

## AuditConfig

**Source:** `crates/core/src/audit.rs`

Audit logging configuration. Logs are written as JSONL with daily rotation.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct AuditConfig {
    pub enabled: bool,
    pub dir: String,
    pub retention_days: u32,
}
```

| Field | Type | Default | YAML key | Description |
|-------|------|---------|----------|-------------|
| `enabled` | `bool` | `false` | `enabled` | Enable audit logging. |
| `dir` | `String` | `"./logs/audit"` | `dir` | Directory for audit log files. |
| `retention_days` | `u32` | `30` | `retention-days` | Number of days to retain audit logs before cleanup. |

### YAML example

```yaml
audit:
  enabled: true
  dir: ./logs/audit
  retention-days: 30
```

---

## DashboardConfig

**Source:** `crates/core/src/config.rs`

Dashboard admin API configuration.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct DashboardConfig {
    pub enabled: bool,
    pub username: String,
    pub password_hash: String,
    pub jwt_secret: Option<String>,
    pub jwt_ttl_secs: u64,
    pub request_log_capacity: usize,
}
```

| Field | Type | Default | YAML key | Description |
|-------|------|---------|----------|-------------|
| `enabled` | `bool` | `false` | `enabled` | Enable the dashboard admin API. |
| `username` | `String` | `"admin"` | `username` | Admin login username. |
| `password_hash` | `String` | `""` | `password-hash` | bcrypt-hashed admin password (e.g., `"$2b$12$..."`). |
| `jwt_secret` | `Option<String>` | `None` | `jwt-secret` | JWT signing secret. Falls back to env `DASHBOARD_JWT_SECRET`. |
| `jwt_ttl_secs` | `u64` | `3600` | `jwt-ttl-secs` | JWT token TTL in seconds. |
| `request_log_capacity` | `usize` | `10_000` | `request-log-capacity` | Request log ring buffer capacity. |

### Key methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `resolve_jwt_secret` | `fn resolve_jwt_secret(&self) -> Option<String>` | Returns `jwt_secret` if set, otherwise reads `DASHBOARD_JWT_SECRET` env var. |

### YAML example

```yaml
dashboard:
  enabled: true
  username: admin
  password-hash: "$2b$12$..."
  jwt-ttl-secs: 3600
  request-log-capacity: 10000
```

---

## DaemonConfig

**Source:** `crates/core/src/config.rs`

Daemon mode configuration.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct DaemonConfig {
    pub pid_file: String,
    pub shutdown_timeout: u64,
}
```

| Field | Type | Default | YAML key | Description |
|-------|------|---------|----------|-------------|
| `pid_file` | `String` | `"./prism.pid"` | `pid-file` | Path to the PID file. |
| `shutdown_timeout` | `u64` | `30` | `shutdown-timeout` | Graceful shutdown timeout in seconds. |

### YAML example

```yaml
daemon:
  pid-file: ./prism.pid
  shutdown-timeout: 30
```

---

## ModelPrice

**Source:** `crates/core/src/cost.rs`

Custom price override for a model (USD per 1M tokens).

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ModelPrice {
    pub input: f64,
    pub output: f64,
}
```

| Field | Type | YAML key | Description |
|-------|------|----------|-------------|
| `input` | `f64` | `input` | Cost per 1M input tokens in USD. |
| `output` | `f64` | `output` | Cost per 1M output tokens in USD. |

### YAML example

```yaml
model-prices:
  custom-model:
    input: 3.0
    output: 15.0
```

---

## PayloadConfig

**Source:** `crates/core/src/payload.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PayloadConfig {
    #[serde(default)]
    pub default: Vec<PayloadRule>,
    #[serde(default)]
    pub r#override: Vec<PayloadRule>,
    #[serde(default)]
    pub filter: Vec<FilterRule>,
}
```

| Field | Type | Default | YAML key | Behavior |
|-------|------|---------|----------|----------|
| `default` | `Vec<PayloadRule>` | `[]` | `default` | Set values only if the field is missing from the request. |
| `override` | `Vec<PayloadRule>` | `[]` | `override` | Always set values, overwriting existing ones. |
| `filter` | `Vec<FilterRule>` | `[]` | `filter` | Remove fields from the request payload. |

Processing order: defaults -> overrides -> filters.

---

## PayloadRule

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PayloadRule {
    pub models: Vec<ModelMatcher>,
    pub params: serde_json::Map<String, Value>,
}
```

| Field | Type | YAML key | Description |
|-------|------|----------|-------------|
| `models` | `Vec<ModelMatcher>` | `models` | Model patterns to match (supports glob). |
| `params` | `Map<String, Value>` | `params` | Dot-separated paths to JSON values (e.g., `"reasoning.effort": "high"`). |

### ModelMatcher

```rust
pub struct ModelMatcher {
    pub name: String,
    pub protocol: Option<String>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | Glob pattern for model name (e.g., `"gemini-*"`, `"*"`). |
| `protocol` | `Option<String>` | Optional target protocol filter (e.g., `"openai"`, `"claude"`). |

---

## FilterRule

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FilterRule {
    pub models: Vec<ModelMatcher>,
    pub params: Vec<String>,
}
```

| Field | Type | YAML key | Description |
|-------|------|----------|-------------|
| `models` | `Vec<ModelMatcher>` | `models` | Model patterns to match. |
| `params` | `Vec<String>` | `params` | Dot-separated paths to remove from payload (e.g., `"generationConfig.responseJsonSchema"`). |

### YAML example (payload section)

```yaml
payload:
  default:
    - models:
        - name: "gemini-*"
      params:
        generationConfig.thinkingConfig.thinkingBudget: 32768
  override:
    - models:
        - name: "gpt-*"
          protocol: openai
      params:
        reasoning.effort: "high"
  filter:
    - models:
        - name: "gemini-2.0-flash*"
      params:
        - generationConfig.responseJsonSchema
```

---

## ProviderKeyEntry

Logical provider-family configuration. A provider entry owns protocol behavior, model catalog, shared routing hints, and zero or more auth profiles.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProviderKeyEntry {
    pub name: String,
    pub format: Format,
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
    pub cloak: CloakConfig,
    #[serde(default)]
    pub wire_api: WireApi,
    #[serde(default = "default_weight")]
    pub weight: u32,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub credential_source: Option<CredentialSource>,
    #[serde(default)]
    pub auth_profiles: Vec<AuthProfileEntry>,
    #[serde(default)]
    pub upstream_presentation: UpstreamPresentationConfig,
    #[serde(default)]
    pub vertex: bool,
    #[serde(default)]
    pub vertex_project: Option<String>,
    #[serde(default)]
    pub vertex_location: Option<String>,
}
```

| Field | Type | Default | YAML key | Description |
|-------|------|---------|----------|-------------|
| `name` | `String` | required | `name` | Stable logical provider name used for routing identity and dashboard APIs. |
| `format` | `Format` | required | `format` | Wire protocol family: `openai`, `claude`, or `gemini`. |
| `api_key` | `String` | `""` | `api-key` | Legacy provider-level secret. If `auth_profiles[]` is empty, Prism exposes it as one implicit API-key auth profile named after the provider. |
| `base_url` | `Option<String>` | `None` | `base-url` | Override provider base URL. Trailing slashes are stripped. |
| `proxy_url` | `Option<String>` | `None` | `proxy-url` | Per-provider proxy URL. Falls back to global `proxy_url`. |
| `prefix` | `Option<String>` | `None` | `prefix` | Legacy provider-level model prefix. When explicit auth profiles exist, profile-level `prefix` is the effective routing prefix. |
| `models` | `Vec<ModelMapping>` | `[]` | `models` | Explicit model list. If empty, all models are accepted. |
| `excluded_models` | `Vec<String>` | `[]` | `excluded-models` | Glob patterns for models to exclude. |
| `headers` | `HashMap<String, String>` | `{}` | `headers` | Shared headers applied to upstream requests. Keys are normalized to lowercase. |
| `disabled` | `bool` | `false` | `disabled` | Disables the provider and all implicit auth derived from it. |
| `cloak` | `CloakConfig` | `CloakMode::Never` | `cloak` | Claude cloaking configuration. |
| `wire_api` | `WireApi` | `Chat` | `wire-api` | Wire API format for OpenAI-family upstreams (`chat` or `responses`). |
| `weight` | `u32` | `1` | `weight` | Legacy provider-level routing weight. Explicit auth profiles can override it per profile. |
| `region` | `Option<String>` | `None` | `region` | Legacy provider-level region hint. Explicit auth profiles can override it per profile. |
| `credential_source` | `Option<CredentialSource>` | `None` | `credential-source` | Optional provider-level secret source for legacy `api_key` auth. |
| `auth_profiles` | `Vec<AuthProfileEntry>` | `[]` | `auth-profiles` | Explicit auth profiles nested under this provider. |
| `upstream_presentation` | `UpstreamPresentationConfig` | defaults | `upstream-presentation` | Shared upstream identity/presentation policy for requests sent through this provider. |
| `vertex` | `bool` | `false` | `vertex` | Enables Vertex AI request shaping for Gemini-family upstreams. |
| `vertex_project` | `Option<String>` | `None` | `vertex-project` | Vertex AI project ID. |
| `vertex_location` | `Option<String>` | `None` | `vertex-location` | Vertex AI region, for example `us-central1`. |

### Key behavior

- `expanded_auth_profiles()` returns explicit `auth_profiles[]` when present.
- If `auth_profiles[]` is empty and `api_key` is set, Prism synthesizes one implicit API-key auth profile using the provider name as the profile ID.
- A provider entry may intentionally have no auth material yet; dashboard auth-profile APIs can attach profiles later.

### YAML example

```yaml
providers:
  - name: "anthropic"
    format: "claude"
    base-url: "https://api.anthropic.com"
    models:
      - id: "claude-sonnet-4-20250514"
        alias: "sonnet"
    auth-profiles:
      - id: "billing"
        mode: "api-key"
        secret: "env://ANTHROPIC_API_KEY"
        weight: 2
      - id: "subscription-main"
        mode: "bearer-token"
        secret: "env://OPENCLAW_SETUP_TOKEN"
        prefix: "anthropic/sub/"
```

---

## AuthProfileEntry

Nested authentication profile for a provider family.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct AuthProfileEntry {
    pub id: String,
    pub mode: AuthMode,
    pub header: AuthHeaderKind,
    pub secret: Option<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    pub expires_at: Option<String>,
    pub account_id: Option<String>,
    pub email: Option<String>,
    pub last_refresh: Option<String>,
    pub headers: HashMap<String, String>,
    pub disabled: bool,
    pub weight: u32,
    pub region: Option<String>,
    pub prefix: Option<String>,
    pub upstream_presentation: UpstreamPresentationConfig,
}
```

| Field | Type | Default | YAML key | Description |
|-------|------|---------|----------|-------------|
| `id` | `String` | required | `id` | Stable profile ID, unique within a provider. |
| `mode` | `AuthMode` | `api-key` | `mode` | Auth material type. |
| `header` | `AuthHeaderKind` | `auto` | `header` | Explicit upstream auth header strategy, or `auto` to derive from mode and provider format. |
| `secret` | `Option<String>` | `None` | `secret` | Static API key or bearer token. Required for `api-key` and `bearer-token` modes unless the profile is disabled. |
| `access_token` | `Option<String>` | `None` | `access-token` | Runtime OAuth access token. Persisted in the auth runtime sidecar store for Codex OAuth profiles. |
| `refresh_token` | `Option<String>` | `None` | `refresh-token` | Runtime OAuth refresh token. Persisted in the auth runtime sidecar store for Codex OAuth profiles. |
| `id_token` | `Option<String>` | `None` | `id-token` | Optional OpenID token returned by the provider. |
| `expires_at` | `Option<String>` | `None` | `expires-at` | RFC3339 token expiry timestamp. |
| `account_id` | `Option<String>` | `None` | `account-id` | Upstream account identifier captured during OAuth completion. |
| `email` | `Option<String>` | `None` | `email` | Upstream account email captured during OAuth completion. |
| `last_refresh` | `Option<String>` | `None` | `last-refresh` | RFC3339 timestamp of the last successful OAuth refresh. |
| `headers` | `HashMap<String, String>` | `{}` | `headers` | Per-profile headers merged into upstream requests. Keys are normalized to lowercase. |
| `disabled` | `bool` | `false` | `disabled` | Disables this auth profile without removing it. |
| `weight` | `u32` | `1` | `weight` | Per-profile routing weight. `0` normalizes to `1`. |
| `region` | `Option<String>` | `None` | `region` | Per-profile region hint for geo-aware routing. |
| `prefix` | `Option<String>` | `None` | `prefix` | Per-profile routing prefix for model names. |
| `upstream_presentation` | `UpstreamPresentationConfig` | defaults | `upstream-presentation` | Per-profile upstream identity/presentation override. |

### Validation notes

- `id` must be non-empty.
- `api-key` and `bearer-token` modes require `secret` unless the profile is disabled.
- `openai-codex-oauth` profiles must not carry a static `secret`.
- `anthropic-claude-subscription` profiles must not carry a static `secret`, are restricted to Claude-format providers, and must target the official `https://api.anthropic.com` base URL.

---

## AuthMode

```rust
pub enum AuthMode {
    ApiKey,
    BearerToken,
    OpenaiCodexOauth,
    AnthropicClaudeSubscription,
}
```

| Variant | YAML value | Meaning |
|---------|------------|---------|
| `ApiKey` | `api-key` | Static key sent with a provider-specific auth header. |
| `BearerToken` | `bearer-token` | Static bearer token, used for subscription/setup-token style flows. |
| `OpenaiCodexOauth` | `openai-codex-oauth` | Refreshable Codex OAuth profile managed through the auth runtime store. |
| `AnthropicClaudeSubscription` | `anthropic-claude-subscription` | Managed Claude setup-token profile stored only in the auth runtime sidecar and always sent as `x-api-key`. |

---

## AuthHeaderKind

```rust
pub enum AuthHeaderKind {
    Auto,
    Bearer,
    XApiKey,
    XGoogApiKey,
}
```

`auto` derives the effective header from the auth mode and provider family:

- OpenAI API keys default to `Authorization: Bearer`.
- Anthropic API keys default to `x-api-key` when using Anthropic-hosted URLs, otherwise `Bearer`.
- Gemini API keys default to `x-goog-api-key`, except Vertex mode which uses `Bearer`.
- Bearer-token and Codex OAuth profiles always default to `Bearer`.

---

## ModelMapping

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ModelMapping {
    pub id: String,
    #[serde(default)]
    pub alias: Option<String>,
}
```

| Field | Type | Default | YAML key | Description |
|-------|------|---------|----------|-------------|
| `id` | `String` | required | `id` | Upstream model ID (supports glob patterns in matching). |
| `alias` | `Option<String>` | `None` | `alias` | Alternate name clients can use. Resolved to `id` before sending upstream. |

---

## CloakConfig

**Source:** `crates/core/src/cloak.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct CloakConfig {
    pub mode: CloakMode,
    pub strict_mode: bool,
    pub sensitive_words: Vec<String>,
    pub cache_user_id: bool,
}
```

| Field | Type | Default | YAML key | Description |
|-------|------|---------|----------|-------------|
| `mode` | `CloakMode` | `Never` | `mode` | When to apply cloaking. |
| `strict_mode` | `bool` | `false` | `strict-mode` | If true, replace user's system prompt entirely; if false, prepend cloak prompt. |
| `sensitive_words` | `Vec<String>` | `[]` | `sensitive-words` | Words to obfuscate by inserting zero-width spaces. |
| `cache_user_id` | `bool` | `false` | `cache-user-id` | Whether to cache the generated `user_id` per API key. |
