# Configuration Types Reference

All configuration types used for YAML config parsing and runtime settings.

**Source:** `crates/core/src/config.rs`, `crates/core/src/payload.rs`, `crates/core/src/cloak.rs`, `crates/core/src/auth_key.rs`, `crates/core/src/cache.rs`, `crates/core/src/audit.rs`, `crates/core/src/circuit_breaker.rs`, `crates/core/src/cost.rs`

---

## Config

The root configuration struct. Loaded from YAML via `Config::load()`. Uses `#[serde(rename_all = "kebab-case", default)]`.

```rust
pub struct Config {
    // Server
    pub host: String,
    pub port: u16,
    pub tls: TlsConfig,

    // Client auth
    pub auth_keys: Vec<AuthKeyEntry>,
    #[serde(skip)]
    pub auth_key_store: AuthKeyStore,    // built from auth_keys during sanitize()

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

    // Timeouts
    pub connect_timeout: u64,
    pub request_timeout: u64,

    // Streaming
    pub streaming: StreamingConfig,
    pub body_limit_mb: usize,

    // Retry
    pub retry: RetryConfig,

    // Payload manipulation
    pub payload: PayloadConfig,

    // Headers
    pub passthrough_headers: Vec<String>,
    pub claude_header_defaults: HashMap<String, String>,
    pub force_model_prefix: bool,
    pub non_stream_keepalive_secs: u64,

    // Cost tracking
    pub model_prices: HashMap<String, ModelPrice>,

    // Rate limiting
    pub rate_limit: RateLimitConfig,

    // Circuit breaker
    pub circuit_breaker: CircuitBreakerConfig,

    // Response cache
    pub cache: CacheConfig,

    // Audit logging
    pub audit: AuditConfig,

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
| `audit` | `AuditConfig` | disabled | `audit` |
| `dashboard` | `DashboardConfig` | disabled | `dashboard` |
| `daemon` | `DaemonConfig` | see below | `daemon` |
| `claude_api_key` | `Vec<ProviderKeyEntry>` | `[]` | `claude-api-key` |
| `openai_api_key` | `Vec<ProviderKeyEntry>` | `[]` | `openai-api-key` |
| `gemini_api_key` | `Vec<ProviderKeyEntry>` | `[]` | `gemini-api-key` |
| `openai_compatibility` | `Vec<ProviderKeyEntry>` | `[]` | `openai-compatibility` |

### Key methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `load` | `fn load(path: &str) -> Result<Self, anyhow::Error>` | Reads YAML, deserializes, sanitizes, and validates. |
| `all_provider_keys` | `fn all_provider_keys(&self) -> impl Iterator<Item = &ProviderKeyEntry>` | Iterates all provider key entries across all provider types. |

### Sanitization (on load)

- Entries with empty `api_key` (and no `credential_source`) are removed.
- Trailing slashes are stripped from `base_url`.
- Header keys are normalized to lowercase.
- `auth_key_store` is built from `auth_keys` for O(1) lookups.

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

Per-credential configuration for a single API key.

```rust
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
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub cloak: CloakConfig,
    #[serde(default)]
    pub wire_api: WireApi,
    #[serde(default = "default_weight")]
    pub weight: u32,
    #[serde(default)]
    pub region: Option<String>,
}
```

| Field | Type | Default | YAML key | Description |
|-------|------|---------|----------|-------------|
| `api_key` | `String` | required | `api-key` | Provider API key. Entries with empty keys are removed during sanitization. |
| `base_url` | `Option<String>` | `None` | `base-url` | Override provider base URL. Trailing slashes are stripped. |
| `proxy_url` | `Option<String>` | `None` | `proxy-url` | Per-credential proxy URL. Falls back to global `proxy_url`. |
| `prefix` | `Option<String>` | `None` | `prefix` | Model name prefix (e.g., `"openai/"`) for namespace isolation. |
| `models` | `Vec<ModelMapping>` | `[]` | `models` | Explicit model list. If empty, all models are accepted. |
| `excluded_models` | `Vec<String>` | `[]` | `excluded-models` | Glob patterns for models to exclude. |
| `headers` | `HashMap<String, String>` | `{}` | `headers` | Extra headers to inject on upstream requests. Keys normalized to lowercase. |
| `disabled` | `bool` | `false` | `disabled` | Disable this credential without removing it. |
| `name` | `Option<String>` | `None` | `name` | Human-readable name for logging/identification. |
| `cloak` | `CloakConfig` | `CloakMode::Never` | `cloak` | Claude cloaking configuration. Only used for Claude provider entries. |
| `wire_api` | `WireApi` | `Chat` | `wire-api` | Wire API format for OpenAI-compatible providers. |
| `weight` | `u32` | `1` | `weight` | Weight for weighted round-robin routing (range 1-100). |
| `region` | `Option<String>` | `None` | `region` | Region identifier for geo-aware routing. |

### YAML example

```yaml
claude-api-key:
  - api-key: "sk-ant-xxx"
    base-url: "https://api.anthropic.com"
    prefix: "claude/"
    name: "primary-claude"
    weight: 2
    region: us-east
    models:
      - id: "claude-sonnet-4-20250514"
        alias: "sonnet"
    excluded-models:
      - "claude-2*"
    cloak:
      mode: auto
      strict-mode: false
      sensitive-words: ["secret"]
      cache-user-id: true
```

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
