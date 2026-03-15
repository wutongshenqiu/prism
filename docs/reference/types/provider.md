# Provider Types Reference

Runtime types for provider execution, credential routing, and format translation.

---

## AuthRecord

**Source:** `crates/core/src/provider.rs`

Runtime credential representation built from `ProviderKeyEntry` during config loading. Carries all data needed to authenticate and route a request to a specific upstream provider.

```rust
#[derive(Clone)]
pub struct AuthRecord {
    pub id: String,
    pub provider: Format,
    pub provider_name: String,
    pub api_key: String,
    pub base_url: Option<String>,
    pub proxy_url: Option<String>,
    pub headers: HashMap<String, String>,
    pub models: Vec<ModelEntry>,
    pub excluded_models: Vec<String>,
    pub prefix: Option<String>,
    pub disabled: bool,
    pub circuit_breaker: Arc<dyn CircuitBreakerPolicy>,
    pub cloak: Option<CloakConfig>,
    pub wire_api: WireApi,
    pub credential_name: Option<String>,
    pub auth_profile_id: String,
    pub auth_mode: AuthMode,
    pub auth_header: AuthHeaderKind,
    pub oauth_state: Option<SharedOAuthTokenState>,
    pub weight: u32,
    pub region: Option<String>,
    pub upstream_presentation: UpstreamPresentationConfig,
    pub vertex: bool,
    pub vertex_project: Option<String>,
    pub vertex_location: Option<String>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | UUID v4 generated at build time. Used to track tried credentials in retry loop. |
| `provider` | `Format` | The provider format (`OpenAI`, `Claude`, or `Gemini`). |
| `provider_name` | `String` | Logical provider-family name from config, used as routing identity. |
| `api_key` | `String` | Static upstream secret. For OAuth profiles this acts as fallback storage when runtime state is empty. |
| `base_url` | `Option<String>` | Custom base URL override. |
| `proxy_url` | `Option<String>` | Per-credential HTTP/SOCKS proxy. |
| `headers` | `HashMap<String, String>` | Extra headers for upstream requests. |
| `models` | `Vec<ModelEntry>` | Explicit model list (empty = accept all). |
| `excluded_models` | `Vec<String>` | Glob patterns for excluded models. |
| `prefix` | `Option<String>` | Model name prefix for namespace isolation. |
| `disabled` | `bool` | Whether this credential is disabled. |
| `circuit_breaker` | `Arc<dyn CircuitBreakerPolicy>` | Circuit breaker instance managing availability state. |
| `cloak` | `Option<CloakConfig>` | Cloak config. Only `Some` for Claude credentials. |
| `wire_api` | `WireApi` | Wire API format (Chat or Responses). |
| `credential_name` | `Option<String>` | Human-readable routing name, currently `provider/profile`. |
| `auth_profile_id` | `String` | Stable auth profile ID within the provider family. |
| `auth_mode` | `AuthMode` | Auth material type (`api-key`, `bearer-token`, `openai-codex-oauth`, or `anthropic-claude-subscription`). |
| `auth_header` | `AuthHeaderKind` | Explicit or derived upstream auth header strategy. |
| `oauth_state` | `Option<SharedOAuthTokenState>` | Shared runtime OAuth state for refreshable profiles. |
| `weight` | `u32` | Weight for weighted round-robin routing (default 1). |
| `region` | `Option<String>` | Region identifier for geo-aware routing. |
| `upstream_presentation` | `UpstreamPresentationConfig` | Per-credential upstream identity/presentation overrides. |
| `vertex` | `bool` | Whether this record targets Vertex AI semantics. |
| `vertex_project` | `Option<String>` | Vertex AI project ID. |
| `vertex_location` | `Option<String>` | Vertex AI location. |

### Key methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `base_url_or_default` | `fn base_url_or_default(&self, default: &str) -> String` | Returns `base_url` or the given default, with trailing slash stripped. |
| `resolved_base_url` | `fn resolved_base_url(&self) -> String` | Returns `base_url` or the canonical default for the record's provider format. |
| `current_secret` | `fn current_secret(&self) -> String` | Returns the runtime OAuth access token when available, otherwise falls back to `api_key`. |
| `resolved_auth_header_kind` | `fn resolved_auth_header_kind(&self) -> AuthHeaderKind` | Resolves `auto` into the concrete header kind sent upstream. |
| `effective_proxy` | `fn effective_proxy<'a>(&'a self, global_proxy: Option<&'a str>) -> Option<&'a str>` | Resolves proxy: entry-level first, then global fallback. |
| `supports_model` | `fn supports_model(&self, model: &str) -> bool` | Checks if this credential handles the given model. Strips prefix, matches against `models` (with glob), checks `excluded_models`. If no explicit model list, accepts everything not excluded. |
| `resolve_model_id` | `fn resolve_model_id(&self, model: &str) -> String` | Strips prefix, resolves alias to actual model ID. |
| `strip_prefix` | `fn strip_prefix<'a>(&self, model: &'a str) -> &'a str` | Removes prefix from model name. Returns original if no prefix match. |
| `prefixed_model_id` | `fn prefixed_model_id(&self, model_id: &str) -> String` | Prepends the configured prefix to a model ID. |
| `is_model_excluded` | `fn is_model_excluded(&self, model: &str) -> bool` | Checks exclusion list using glob matching. |
| `name` | `fn name(&self) -> Option<&str>` | Returns the credential's human-readable name. |
| `is_available` | `fn is_available(&self) -> bool` | Returns `false` if disabled or circuit breaker denies execution (`circuit_breaker.can_execute()` returns false). |
| `circuit_state` | `fn circuit_state(&self) -> CircuitState` | Returns the current circuit breaker state (`Closed`, `Open`, or `HalfOpen`). |

---

## ModelEntry

**Source:** `crates/core/src/provider.rs`

```rust
#[derive(Debug, Clone)]
pub struct ModelEntry {
    pub id: String,
    pub alias: Option<String>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | Upstream model identifier (supports glob in matching). |
| `alias` | `Option<String>` | Client-facing alias that resolves to `id`. |

---

## ProviderRequest

**Source:** `crates/core/src/provider.rs`

Encapsulates a request to be sent to an upstream provider.

```rust
#[derive(Debug, Clone)]
pub struct ProviderRequest {
    pub model: String,
    pub payload: Bytes,
    pub source_format: Format,
    pub stream: bool,
    pub headers: HashMap<String, String>,
    pub original_request: Option<Bytes>,
    pub responses_passthrough: bool,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `model` | `String` | Resolved actual model ID (after alias/prefix resolution). |
| `payload` | `Bytes` | Translated request body in the target provider's format. |
| `source_format` | `Format` | The format of the original client request. |
| `stream` | `bool` | Whether the client requested streaming. |
| `headers` | `HashMap<String, String>` | Extra request headers (e.g., claude-header-defaults for cloaking). |
| `original_request` | `Option<Bytes>` | Original request body, preserved for response translation. |
| `responses_passthrough` | `bool` | When `true`, the payload is already in OpenAI Responses format and should be forwarded without further conversion. |

---

## ProviderResponse

**Source:** `crates/core/src/provider.rs`

```rust
#[derive(Debug, Clone)]
pub struct ProviderResponse {
    pub payload: Bytes,
    pub headers: HashMap<String, String>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `payload` | `Bytes` | Response body from upstream. |
| `headers` | `HashMap<String, String>` | Response headers from upstream (all headers extracted). |

---

## StreamChunk

**Source:** `crates/core/src/provider.rs`

A single chunk in a streaming (SSE) response.

```rust
#[derive(Debug, Clone)]
pub struct StreamChunk {
    pub event_type: Option<String>,
    pub data: String,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `event_type` | `Option<String>` | SSE event type (e.g., `"message_start"` for Claude). `None` for OpenAI-style data-only events. |
| `data` | `String` | The JSON data payload. |

---

## StreamResult

**Source:** `crates/core/src/provider.rs`

The result of a streaming provider execution.

```rust
pub struct StreamResult {
    pub headers: HashMap<String, String>,
    pub stream: Pin<Box<dyn Stream<Item = Result<StreamChunk, ProxyError>> + Send>>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `headers` | `HashMap<String, String>` | Upstream response headers. |
| `stream` | `Pin<Box<dyn Stream<...>>>` | Async stream of `StreamChunk` results. |

---

## ModelInfo

**Source:** `crates/core/src/provider.rs`

Model metadata exposed via `/v1/models`.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub provider: String,
    pub owned_by: String,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | Model identifier (alias preferred over raw ID). |
| `provider` | `String` | Logical provider name exposed by the router (for example `openai`, `anthropic`, `openai-codex`). |
| `owned_by` | `String` | Same as `provider`, for OpenAI-compatible list responses. |

---

## ProviderExecutor trait

**Source:** `crates/core/src/provider.rs`

Trait implemented by each provider (OpenAI, Claude, Gemini, OpenAI-compat) to handle upstream API communication.

```rust
#[async_trait]
pub trait ProviderExecutor: Send + Sync {
    fn identifier(&self) -> &str;
    fn native_format(&self) -> Format;

    async fn execute(
        &self,
        auth: &AuthRecord,
        request: ProviderRequest,
    ) -> Result<ProviderResponse, ProxyError>;

    async fn execute_stream(
        &self,
        auth: &AuthRecord,
        request: ProviderRequest,
    ) -> Result<StreamResult, ProxyError>;

    fn supported_models(&self, auth: &AuthRecord) -> Vec<ModelInfo>;
}
```

| Method | Return | Description |
|--------|--------|-------------|
| `identifier()` | `&str` | Executor registry key (for example `"openai"`, `"claude"`, or `"gemini"`). |
| `native_format()` | `Format` | The provider's native API format. |
| `execute()` | `Result<ProviderResponse, ProxyError>` | Non-streaming request execution. |
| `execute_stream()` | `Result<StreamResult, ProxyError>` | Streaming request execution. |
| `supported_models()` | `Vec<ModelInfo>` | List of models available through this auth record. |

### Registered executors (`crates/provider/src/lib.rs`)

| Key | Type | Native Format | Notes |
|-----|------|--------------|-------|
| `"openai"` | `openai_compat::OpenAICompatExecutor` | `Format::OpenAI` | Handles OpenAI-format upstreams, including custom base URLs and `responses` mode |
| `"claude"` | `claude::ClaudeExecutor` | `Format::Claude` | |
| `"gemini"` | `gemini::GeminiExecutor` | `Format::Gemini` | |

---

## CredentialRouter

**Source:** `crates/provider/src/routing.rs`

Thread-safe credential store that selects the appropriate credential for each request based on provider, model, routing strategy, and circuit breaker state.

```rust
pub struct CredentialRouter {
    credentials: RwLock<HashMap<String, Vec<AuthRecord>>>,
    credential_index: RwLock<HashMap<String, (String, usize)>>,
    runtime_oauth_states: RwLock<HashMap<String, OAuthTokenState>>,
    counters: RwLock<HashMap<String, AtomicUsize>>,
    strategy: RwLock<CredentialStrategy>,
    latency_ewma: RwLock<HashMap<String, f64>>,
    ewma_alpha: RwLock<f64>,
    cb_config: RwLock<CircuitBreakerConfig>,
    cooldowns: DashMap<String, QuotaCooldown>,
}
```

### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(strategy: CredentialStrategy) -> Self` | Create a new router with the given strategy. |
| `set_oauth_states` | `fn set_oauth_states(&self, states: HashMap<String, OAuthTokenState>)` | Injects runtime OAuth state keyed by `provider/profile` before rebuilding auth records. |
| `pick` | `fn pick(&self, provider_name: &str, model: &str, tried: &[String], client_region: Option<&str>, allowed_credentials: &[String]) -> Option<AuthRecord>` | Picks the next available credential under a logical provider name. Filters by model support, tried IDs, cooldown, and optional credential allowlist. |
| `record_latency` | `fn record_latency(&self, credential_id: &str, latency_ms: f64)` | Record request latency for EWMA calculation (used by latency-aware routing). |
| `record_success` | `fn record_success(&self, auth_id: &str)` | Report a successful request to the credential's circuit breaker. |
| `record_failure` | `fn record_failure(&self, auth_id: &str)` | Report a failed request to the credential's circuit breaker. May trip the circuit open. |
| `set_quota_cooldown` | `fn set_quota_cooldown(&self, credential_id: &str, duration: Duration)` | Temporarily removes a credential from selection after quota-related failures. |
| `circuit_breaker_states` | `fn circuit_breaker_states(&self) -> Vec<(String, bool)>` | Get circuit breaker availability for all credentials. Returns `(credential_name, can_execute)`. |
| `update_from_config` | `fn update_from_config(&self, config: &Config)` | Rebuilds auth records from unified `providers[]`, preserving circuit breaker and shared OAuth state where possible. |
| `all_models` | `fn all_models(&self) -> Vec<ModelInfo>` | Lists all unique models across available credentials, deduplicated by the exposed model ID. |
| `model_has_prefix` | `fn model_has_prefix(&self, model: &str) -> bool` | Check if any available credential with a prefix supports this model. Used for `force_model_prefix` enforcement. |
| `resolve_providers` | `fn resolve_providers(&self, model: &str) -> Vec<(String, Format)>` | Return all logical providers that have at least one available credential supporting the model. |
| `credential_map` | `fn credential_map(&self) -> HashMap<String, Vec<AuthRecord>>` | Returns a snapshot of auth records grouped by logical provider name. |

---

## TranslatorRegistry

**Source:** `crates/translator/src/lib.rs`

Registry of format translation functions for converting requests and responses between provider formats.

```rust
pub struct TranslatorRegistry {
    requests: HashMap<(Format, Format), RequestTransformFn>,
    responses: HashMap<(Format, Format), ResponseTransform>,
}
```

### Function type aliases

```rust
pub type RequestTransformFn =
    fn(model: &str, raw_json: &[u8], stream: bool) -> Result<Vec<u8>, ProxyError>;

pub type StreamTransformFn = fn(
    model: &str,
    original_req: &[u8],
    event_type: Option<&str>,
    data: &[u8],
    state: &mut TranslateState,
) -> Result<Vec<String>, ProxyError>;

pub type NonStreamTransformFn =
    fn(model: &str, original_req: &[u8], data: &[u8]) -> Result<String, ProxyError>;
```

### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new() -> Self` | Create an empty registry. |
| `register` | `fn register(&mut self, from: Format, to: Format, request: RequestTransformFn, response: ResponseTransform)` | Register a translation pair. |
| `translate_request` | `fn translate_request(&self, from: Format, to: Format, model: &str, raw_json: &[u8], stream: bool) -> Result<Vec<u8>, ProxyError>` | Translate a request. If `from == to`, replaces only the `model` field (alias resolution). |
| `translate_stream` | `fn translate_stream(&self, from: Format, to: Format, model: &str, orig_req: &[u8], event_type: Option<&str>, data: &[u8], state: &mut TranslateState) -> Result<Vec<String>, ProxyError>` | Translate a streaming response chunk. Returns multiple SSE lines. Passes `[DONE]` sentinel through unchanged. |
| `translate_non_stream` | `fn translate_non_stream(&self, from: Format, to: Format, model: &str, orig_req: &[u8], data: &[u8]) -> Result<String, ProxyError>` | Translate a non-streaming response body. |
| `has_response_translator` | `fn has_response_translator(&self, from: Format, to: Format) -> bool` | Check if a response translator exists (and formats differ). |

### Registered translations (`build_registry()`)

| From | To | Request translator | Response translator |
|------|----|--------------------|---------------------|
| `OpenAI` | `Claude` | `openai_to_claude::translate_request` | `claude_to_openai::translate_stream` / `translate_non_stream` |
| `OpenAI` | `Gemini` | `openai_to_gemini::translate_request` | `gemini_to_openai::translate_stream` / `translate_non_stream` |
| `Gemini` | `OpenAI` | `gemini_to_openai_request::translate_request` | `openai_to_gemini_response::translate_stream` / `translate_non_stream` |
| `Gemini` | `Claude` | Request-only chain: Gemini -> OpenAI -> Claude | No dedicated response translator |
| `Claude` | `OpenAI` | `claude_to_openai_request::translate_request` | `openai_to_claude_response::translate_stream` / `translate_non_stream` |
| `Claude` | `Gemini` | Request-only chain: Claude -> OpenAI -> Gemini | No dedicated response translator |

Same-format pairs (for example `OpenAI -> OpenAI`) are handled implicitly: only the `model` field is replaced. When no direct translator exists for a cross-format pair, the registry falls back to passthrough behavior unless a request-only chained translation is explicitly registered.

---

## TranslateState

**Source:** `crates/translator/src/lib.rs`

Mutable state accumulated during stream translation. Passed to each `StreamTransformFn` invocation for a single response.

```rust
#[derive(Debug, Default)]
pub struct TranslateState {
    pub response_id: String,
    pub model: String,
    pub created: i64,
    pub current_tool_call_index: Option<usize>,
    pub current_content_index: Option<usize>,
    pub sent_role: bool,
    pub input_tokens: u64,
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `response_id` | `String` | `""` | Generated response ID for the translated response. |
| `model` | `String` | `""` | Model name for the translated response. |
| `created` | `i64` | `0` | Unix timestamp for the translated response. |
| `current_tool_call_index` | `Option<usize>` | `None` | Tracks the current tool call index during stream assembly. |
| `current_content_index` | `Option<usize>` | `None` | Tracks the current content block index during stream assembly. |
| `sent_role` | `bool` | `false` | Whether the assistant role delta has been emitted. |
| `input_tokens` | `u64` | `0` | Accumulated input token count from upstream. |

---

## ExecutorRegistry

**Source:** `crates/provider/src/lib.rs`

Registry of provider executor instances, keyed by name.

```rust
pub struct ExecutorRegistry {
    executors: HashMap<String, Arc<dyn ProviderExecutor>>,
}
```

### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `get` | `fn get(&self, name: &str) -> Option<Arc<dyn ProviderExecutor>>` | Look up executor by name. |
| `get_by_format` | `fn get_by_format(&self, format: Format) -> Option<Arc<dyn ProviderExecutor>>` | Look up executor by native format. |
| `all` | `fn all(&self) -> impl Iterator<Item = (&String, &Arc<dyn ProviderExecutor>)>` | Iterate all registered executors. |

---

## RequestContext

**Source:** `crates/core/src/context.rs`

Per-request metadata injected as an axum `Extension` by the request context middleware.

```rust
#[derive(Debug, Clone)]
pub struct RequestContext {
    pub request_id: String,
    pub start_time: Instant,
    pub client_ip: Option<String>,
    pub api_key_id: Option<String>,
    pub tenant_id: Option<String>,
    pub auth_key: Option<AuthKeyEntry>,
    pub client_region: Option<String>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `request_id` | `String` | UUID v4 generated per request. |
| `start_time` | `Instant` | When the request was received. |
| `client_ip` | `Option<String>` | Extracted from `X-Forwarded-For` or `X-Real-IP` headers. |
| `api_key_id` | `Option<String>` | Masked API key ID (set by auth middleware after key validation). |
| `tenant_id` | `Option<String>` | Tenant ID from the matching `AuthKeyEntry`. |
| `auth_key` | `Option<AuthKeyEntry>` | Full auth key entry (for per-key rate limits and model access checks). |
| `client_region` | `Option<String>` | Client region for geo-aware routing (extracted from headers or config). |

### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(client_ip: Option<String>) -> Self` | Create with a new UUID and current time. |
| `elapsed_ms` | `fn elapsed_ms(&self) -> u128` | Milliseconds elapsed since `start_time`. |
