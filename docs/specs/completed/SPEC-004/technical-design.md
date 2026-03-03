# Technical Design: Configuration System & Hot-Reload

| Field     | Value                              |
|-----------|------------------------------------|
| Spec ID   | SPEC-004                           |
| Title     | Configuration System & Hot-Reload  |
| Author    | Prism Team                      |
| Status    | Completed                          |
| Created   | 2026-02-27                         |
| Updated   | 2026-02-27                         |

## Overview

The configuration system is built around a `Config` struct deserialized from YAML, wrapped in `Arc<ArcSwap<Config>>` for lock-free concurrent access. A `ConfigWatcher` monitors the config file for changes, debounces events, deduplicates by SHA256 hash, and atomically swaps in new config. CLI arguments and environment variables provide override capabilities. See PRD (SPEC-004) for requirements.

## Backend Implementation

### Module Structure

```
prism/src/main.rs              -- CLI args (clap), config loading, ArcSwap setup, watcher start
crates/core/src/config.rs         -- Config struct, sub-configs, ConfigWatcher, load/validate/sanitize
```

### Key Types

#### Config (root)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct Config {
    // Server
    pub host: String,                                // default: "0.0.0.0"
    pub port: u16,                                   // default: 8317
    pub tls: TlsConfig,

    // Client auth
    pub api_keys: Vec<String>,
    pub api_keys_set: HashSet<String>,               // #[serde(skip)] -- built during sanitize

    // Global proxy
    pub proxy_url: Option<String>,

    // Debug & logging
    pub debug: bool,
    pub logging_to_file: bool,
    pub log_dir: Option<String>,

    // Routing
    pub routing: RoutingConfig,
    pub request_retry: u32,                          // default: 3
    pub max_retry_interval: u64,                     // default: 30

    // Timeouts (seconds)
    pub connect_timeout: u64,                        // default: 30
    pub request_timeout: u64,                        // default: 300

    // Streaming
    pub streaming: StreamingConfig,

    // Request body size limit
    pub body_limit_mb: usize,                        // default: 10

    // Retry
    pub retry: RetryConfig,

    // Payload manipulation
    pub payload: PayloadConfig,

    // Response header forwarding
    pub passthrough_headers: Vec<String>,

    // Claude header injection during cloaking
    pub claude_header_defaults: HashMap<String, String>,

    // Model prefix enforcement
    pub force_model_prefix: bool,                    // default: false

    // Non-stream keepalive
    pub non_stream_keepalive_secs: u64,              // default: 0 (disabled)

    // Provider credentials
    pub claude_api_key: Vec<ProviderKeyEntry>,
    pub openai_api_key: Vec<ProviderKeyEntry>,
    pub gemini_api_key: Vec<ProviderKeyEntry>,
    pub openai_compatibility: Vec<ProviderKeyEntry>,
}
```

#### Sub-configs

```rust
pub struct TlsConfig { enable: bool, cert: Option<String>, key: Option<String> }
pub struct RoutingConfig { strategy: RoutingStrategy }  // RoundRobin | FillFirst
pub struct StreamingConfig { keepalive_seconds: u64, bootstrap_retries: u32 }
pub struct RetryConfig { max_retries: u32, max_backoff_secs: u64, cooldown_429_secs: u64,
                         cooldown_5xx_secs: u64, cooldown_network_secs: u64 }
```

#### ProviderKeyEntry

```rust
pub struct ProviderKeyEntry {
    pub api_key: String,
    pub base_url: Option<String>,
    pub proxy_url: Option<String>,
    pub prefix: Option<String>,
    pub models: Vec<ModelMapping>,
    pub excluded_models: Vec<String>,
    pub headers: HashMap<String, String>,
    pub disabled: bool,
    pub name: Option<String>,
    pub cloak: CloakConfig,
    pub wire_api: WireApi,
}
```

#### CLI Args

```rust
#[derive(Parser)]
#[command(name = "prism", version, about = "Prism — AI API Proxy Gateway")]
struct Cli {
    #[arg(short, long, default_value = "config.yaml", env = "PRISM_CONFIG")]
    config: String,
    #[arg(long, env = "PRISM_HOST")]
    host: Option<String>,
    #[arg(long, env = "PRISM_PORT")]
    port: Option<u16>,
    #[arg(long, default_value = "info", env = "PRISM_LOG_LEVEL")]
    log_level: String,
}
```

### Config::load() Flow

1. Read YAML file contents via `std::fs::read_to_string`
2. Deserialize via `serde_yml::from_str` into `Config`
3. Call `sanitize()`:
   - Remove entries with empty `api_key`, deduplicate by `api_key`, strip trailing slashes from `base_url`, lowercase header keys
   - Build `api_keys_set` HashSet from `api_keys` for O(1) lookups
4. Call `validate()`:
   - If TLS enabled, ensure `cert` and `key` paths are set
   - Validate all `proxy_url` values across provider entries and global proxy
5. Return the validated `Config`

### Config::validate()

- Checks TLS configuration completeness
- Validates proxy URLs for all provider key entries via `crate::proxy::validate_proxy_url`
- Returns `anyhow::Error` on validation failure

### Config::sanitize()

- Calls `sanitize_entries()` on each provider key list (claude, openai, gemini, openai-compat)
- Builds `api_keys_set` HashSet for auth middleware

### sanitize_entries()

- `entries.retain(|e| !e.api_key.is_empty())` -- remove empty keys
- Deduplicate by `api_key` using a `HashSet<String>`
- Strip trailing `/` from `base_url`
- Lowercase all header keys

### all_provider_keys()

```rust
pub fn all_provider_keys(&self) -> impl Iterator<Item = &ProviderKeyEntry> {
    self.claude_api_key.iter()
        .chain(self.openai_api_key.iter())
        .chain(self.gemini_api_key.iter())
        .chain(self.openai_compatibility.iter())
}
```

Aggregates all provider key entries across all four provider sections. Used for validation and admin endpoints.

### ConfigWatcher

The `ConfigWatcher` watches the config file and atomically swaps in new configs.

**Architecture:**

```
[notify::RecommendedWatcher] --event--> [mpsc channel] --recv--> [tokio task]
                                                                    |
                                                            debounce 150ms
                                                                    |
                                                         SHA256 hash check
                                                                    |
                                                           Config::load()
                                                                    |
                                                  on_reload callback + ArcSwap::store
```

**Key behaviors:**

1. **File watching:** Uses `notify::recommended_watcher` with `RecursiveMode::NonRecursive`. Triggers on `is_modify()` or `is_create()` events.
2. **Debouncing:** Sets a deadline 150ms in the future on each event. Processing only occurs after 150ms of no new events. This batches rapid saves (e.g., editor write-then-rename).
3. **SHA256 deduplication:** Computes `sha2::Sha256::digest` of file contents. Skips reload if hash matches the last successful load. Prevents redundant processing when file is touched without content changes.
4. **Atomic swap:** On successful reload, calls `config.store(Arc::new(new_cfg))` via ArcSwap. All readers instantly see the new config on their next `config.load()`.
5. **Callback:** Invokes `on_reload(&new_cfg)` before storing, allowing the caller to update dependent state (e.g., `CredentialRouter::update_from_config`).
6. **Error handling:** Failed reloads log an error but leave the previous valid config in place.

### ArcSwap Integration

```rust
// In main.rs -- wrap config in ArcSwap
let config = Arc::new(ArcSwap::from_pointee(config));

// Reading (lock-free, never blocks)
let cfg = state.config.load();

// Writing (atomic swap, from ConfigWatcher)
config.store(Arc::new(new_cfg));
```

- `load()` returns a `Guard<Arc<Config>>` that is cheaply cloneable
- Writers never block readers; no mutex contention
- Each handler gets a consistent snapshot of config for the duration of its request

### Startup Flow

1. `dotenvy::dotenv().ok()` -- load `.env` file
2. `Cli::parse()` -- parse CLI args (with env fallbacks)
3. `Config::load(&cli.config)` -- load YAML, sanitize, validate; fall back to `Config::default()` on failure
4. Apply CLI overrides (`host`, `port`)
5. Build provider components (executor registry, credential router, translator registry)
6. Call `router.update_from_config(&config)` to initialize credentials from the loaded config
7. Wrap config in `Arc<ArcSwap<Config>>`
8. Start `ConfigWatcher` with callback that calls `router.update_from_config`
9. Bind and serve (HTTP or HTTPS depending on TLS config)

## Configuration Changes

```yaml
# config.yaml -- all fields use kebab-case
host: "0.0.0.0"
port: 8317
tls:
  enable: false
  cert: null
  key: null
api-keys:
  - "sk-proxy-xxx"
proxy-url: null
debug: false
routing:
  strategy: round-robin  # or fill-first
retry:
  max-retries: 3
  max-backoff-secs: 30
  cooldown-429-secs: 60
  cooldown-5xx-secs: 15
  cooldown-network-secs: 10
streaming:
  keepalive-seconds: 15
  bootstrap-retries: 1
body-limit-mb: 10
connect-timeout: 30
request-timeout: 300
force-model-prefix: false
non-stream-keepalive-secs: 0
passthrough-headers: []
claude-header-defaults: {}
payload: {}
claude-api-key: []
openai-api-key: []
gemini-api-key: []
openai-compatibility: []
```

## Provider Compatibility

| Provider | Supported | Notes |
|----------|-----------|-------|
| OpenAI   | Yes       | Keys under `openai-api-key` |
| Claude   | Yes       | Keys under `claude-api-key`, supports cloak config per entry |
| Gemini   | Yes       | Keys under `gemini-api-key` |
| Compat   | Yes       | Keys under `openai-compatibility`, supports `wire-api` field |

## Test Strategy

- **Unit tests:** `test_default_config` validates defaults, `test_sanitize_entries` verifies dedup/normalization, `test_yaml_deserialization` checks serde round-trip
- **Manual verification:** Edit `config.yaml` while running, observe "Configuration reloaded successfully" log
