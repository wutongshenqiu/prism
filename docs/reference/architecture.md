# Architecture Reference

System architecture, crate dependencies, request lifecycle, and key design patterns.

---

## Crate Dependency Diagram

```
prism (binary, src/main.rs)
├── prism-server (crates/server)
│   ├── prism-core (crates/core)
│   ├── prism-provider (crates/provider)
│   │   └── prism-core
│   └── prism-translator (crates/translator)
│       └── prism-core
├── prism-core
├── prism-provider
└── prism-translator
```

### Crate responsibilities

| Crate | Path | Purpose |
|-------|------|---------|
| `prism` | `src/` | Binary entry point. CLI arg parsing (clap), config loading, executor/translator/router initialization, server startup, TLS setup, config watcher. |
| `prism-core` | `crates/core/` | Foundation types shared by all crates: `Config`, `ProxyError`, `Format`, `AuthRecord`, `ProviderExecutor` trait, `Metrics`, `RequestContext`, `PayloadConfig`, `CloakConfig`, glob matching, proxy URL handling. |
| `prism-provider` | `crates/provider/` | Provider executor implementations (OpenAI, Claude, Gemini, OpenAI-compat), `CredentialRouter`, `ExecutorRegistry`, SSE stream parsing, HTTP client construction. |
| `prism-translator` | `crates/translator/` | Format translation between provider APIs: `TranslatorRegistry`, `TranslateState`, OpenAI<->Claude and OpenAI<->Gemini request/response translators. |
| `prism-server` | `crates/server/` | Axum router, HTTP handlers, authentication middleware, request context/logging middleware, dispatch engine, SSE streaming response builder. |

---

## Request Lifecycle

Step-by-step flow from HTTP request to response.

```
Client
  |
  v
[1] Axum Router (TraceLayer -> CorsLayer -> RequestContext -> RequestLogging)
  |
  v
[2] Auth Middleware (Bearer token / x-api-key validation)
  |
  v
[3] RequestBodyLimitLayer (body size check)
  |
  v
[4] Handler (chat_completions / messages / responses / models)
  |
  v
[5] Parse Request (extract model, stream flag, User-Agent)
  |
  v
[6] Dispatch Engine
  |
  +--[6a] Model ACL check (auth key allowed_models vs requested model)
  |
  +--[6b] Cache lookup (non-stream requests with temperature=0)
  |       On HIT: return cached response immediately
  |       On MISS: continue to provider dispatch
  |
  +--[6c] Resolve Providers (router.resolve_providers(model))
  |       Find all provider formats with available credentials for this model
  |
  +--[6d] Enforce model prefix (if force_model_prefix is enabled)
  |
  +--[6e] Retry Loop (up to max_retries attempts)
  |   |
  |   +-- For each provider format:
  |       |
  |       +--[6f] Pick Credential (router.pick with round-robin or fill-first)
  |       |       Skip already-tried credential IDs
  |       |       Skip credentials with open circuit breaker
  |       |
  |       +--[6g] Resolve Model ID (strip prefix, resolve alias -> actual ID)
  |       |
  |       +--[6h] Translate Request (TranslatorRegistry: source -> target format)
  |       |       Same format: only replace model field
  |       |       Different format: full translation (e.g., OpenAI -> Claude)
  |       |
  |       +--[6i] Apply Payload Rules (default / override / filter)
  |       |
  |       +--[6j] Apply Cloaking (Claude targets only, if configured)
  |       |       Inject system prompt, user_id, obfuscate sensitive words
  |       |
  |       +--[6k] Execute (ProviderExecutor.execute or execute_stream)
  |       |
  |       +--[6l] On success:
  |       |       Streaming: translate stream chunks, build SSE response
  |       |       Non-stream: translate response body, build JSON response
  |       |       Forward passthrough_headers from upstream
  |       |       Record circuit breaker success
  |       |
  |       +--[6m] On error:
  |               Record circuit breaker failure (429, 5xx, network errors)
  |               Add credential ID to tried list
  |               Continue to next provider/attempt
  |
  +--[6n] Exponential backoff with jitter between retry rounds
  |       cap = min(2^attempt, max_backoff_secs)
  |       sleep = base + random(0, cap * jitter_factor)
  |       default jitter_factor=1.0 (full jitter)
  |
  v
[7] Response returned to client
```

---

## Key Design Patterns

### ArcSwap for Hot-Reload

Configuration is stored in `Arc<ArcSwap<Config>>`, allowing atomic, lock-free reads from all request handlers while the `ConfigWatcher` can swap in new configurations.

**Flow:**
1. `ConfigWatcher` monitors the YAML file using `notify` (filesystem events).
2. On change: 150ms debounce, SHA-256 dedup to avoid redundant reloads.
3. `Config::load()` parses, sanitizes, and validates the new config.
4. `config.store(Arc::new(new_cfg))` atomically publishes the new config.
5. `on_reload` callback updates `CredentialRouter` via `update_from_config()`.
6. All subsequent `config.load()` calls see the new config immediately.

**Source:** `crates/core/src/config.rs` (`ConfigWatcher::start`)

---

### Circuit Breaker per Credential

When a provider returns an error, the failing credential's circuit breaker records the failure. After enough consecutive failures, the circuit trips open and the credential is temporarily removed from the rotation pool.

**Mechanism:**
- `AuthRecord.circuit_breaker: Arc<dyn CircuitBreakerPolicy>` -- pluggable policy per credential.
- Default implementation: `ThreeStateCircuitBreaker` with states `Closed → Open → HalfOpen → Closed`.
- `CredentialRouter.record_failure(auth_id)` calls `circuit_breaker.record_failure()`.
- `CredentialRouter.pick()` skips credentials where `circuit_breaker.can_execute()` returns `false`.

**Three-state transitions:**

| State | Behavior |
|-------|----------|
| `Closed` | Normal operation. Failures increment the counter. Transitions to `Open` after `failure_threshold` consecutive failures. |
| `Open` | All requests blocked. After `cooldown_secs`, transitions to `HalfOpen`. |
| `HalfOpen` | Up to `half_open_max_probes` requests allowed. Success → `Closed`, failure → `Open`. |

**Configuration** (via `CircuitBreakerConfig`):

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | `true` | Enable/disable circuit breaker |
| `failure_threshold` | `5` | Failures before tripping open |
| `cooldown_secs` | `30` | Seconds before transitioning to half-open |
| `half_open_max_probes` | `1` | Max probe requests in half-open state |
| `rolling_window_secs` | `60` | Rolling window for failure counting |

**Error handling in dispatch:** `handle_retry_error()` records circuit breaker failures for HTTP 429, 5xx, and network errors.

**Source:** `crates/core/src/circuit_breaker.rs`, `crates/provider/src/routing.rs`, `crates/server/src/dispatch.rs`

---

### Round-Robin via AtomicUsize Counters

When `RoutingStrategy::RoundRobin` is selected, credentials are distributed using lock-free atomic counters.

**Mechanism:**
- `CredentialRouter.counters: RwLock<HashMap<String, AtomicUsize>>` stores one counter per `"provider:model"` key.
- On each `pick()` call, the counter is incremented with `fetch_add(1, Relaxed)`.
- The candidate index is `counter % candidates.len()`.
- The read lock fast-path avoids contention; write lock is taken only for new keys.

**Why Relaxed ordering:** The counter only needs monotonicity within a single thread's view. Slight reordering across threads is acceptable for load balancing -- perfect fairness is not required.

**Source:** `crates/provider/src/routing.rs`

---

### TranslateState for Stateful Stream Translation

Streaming responses require state accumulation across chunks because:
- OpenAI and Claude use different streaming event structures
- Tool call assembly requires tracking the current index
- The response ID, model, and created timestamp must be consistent across all translated chunks
- Token usage may be reported in a final chunk

**Implementation:**
- `TranslateState` is created as `Default` at the start of each stream.
- It is passed as `&mut TranslateState` to each `StreamTransformFn` call.
- Fields like `response_id`, `model`, `created` are populated from the first chunk.
- `current_tool_call_index` and `current_content_index` track assembly progress.
- `sent_role` prevents duplicate role deltas.
- `input_tokens` accumulates token counts from upstream events.

**Source:** `crates/translator/src/lib.rs`

---

### Dispatch Retry Loop with Provider Failover

The dispatch engine implements a two-level retry loop that provides both intra-provider and cross-provider failover.

```
for attempt in 0..max_retries {
    for target_format in providers {
        pick credential -> translate -> execute
        on error: record circuit breaker failure, try next
    }
    exponential backoff with jitter
}
```

**Key properties:**
- **Cross-provider failover:** If all OpenAI credentials are rate-limited, the next iteration tries Claude or Gemini credentials that also support the model.
- **Credential exclusion:** The `tried` list prevents re-picking the same credential within a single dispatch.
- **Streaming bootstrap limit:** For streaming requests, a separate `bootstrap_retries` config limits retries before the first byte is sent to the client (since once SSE headers are sent, retrying is not possible).
- **Exponential backoff with jitter:** Between retry rounds, `cap = min(2^attempt, max_backoff_secs)`, then `sleep = base + random(0, cap * jitter_factor)` where `base = cap * (1 - jitter_factor)`. Default `jitter_factor` is 1.0 (full jitter: `sleep = random(0, cap)`).

**Non-stream keepalive mode:** When `non_stream_keepalive_secs > 0`, the dispatch races the upstream execute against a timer. If the timer fires first, it switches to a chunked response body that sends periodic whitespace (` `) to prevent intermediate proxy timeouts. The final response payload is appended when it arrives. Leading whitespace is valid JSON and is ignored by parsers.

**Source:** `crates/server/src/dispatch.rs`
