# API Surface Reference

All HTTP endpoints, authentication, middleware, and request/response formats.

**Source:** `crates/server/src/lib.rs`, `crates/server/src/handler/`, `crates/server/src/auth.rs`

---

## Endpoints

### Public routes (no auth required)

#### GET /health

Health check endpoint.

**Response:**
```json
{
  "status": "ok",
  "version": "<CARGO_PKG_VERSION>"
}
```

> Note: `version` is derived from `env!("CARGO_PKG_VERSION")` at compile time.

**Source:** `crates/server/src/handler/health.rs`

---

#### GET /metrics

Returns in-memory metrics snapshot with atomic counters (JSON format).

---

#### GET /metrics/prometheus

Returns metrics in Prometheus text exposition format. Includes request counts by model/provider, latency histograms, token usage, cost, cache hit/miss, and circuit breaker states.

**Response:** `text/plain; version=0.0.4`

**Source:** `crates/server/src/handler/health.rs`, `crates/core/src/prometheus.rs`

**Response:**
```json
{
  "total_requests": 1234,
  "total_errors": 5,
  "total_input_tokens": 50000,
  "total_output_tokens": 100000,
  "latency_ms": {
    "<100": 500,
    "100-499": 300,
    "500-999": 200,
    "1000-4999": 150,
    "5000-29999": 80,
    ">=30000": 4
  },
  "by_model": {
    "gpt-4": 600,
    "claude-sonnet-4-20250514": 400
  },
  "by_provider": {
    "openai": 600,
    "claude": 400
  }
}
```

**Source:** `crates/server/src/handler/health.rs`, `crates/core/src/metrics.rs`

---

### Admin routes (no auth required, read-only)

#### GET /admin/config

Returns sanitized configuration (no API keys exposed).

**Response:**
```json
{
  "host": "0.0.0.0",
  "port": 8317,
  "tls": { "enable": false },
  "api_keys_count": 2,
  "routing": { "strategy": "round-robin" },
  "retry": {
    "max-retries": 3,
    "max-backoff-secs": 30,
    "cooldown-429-secs": 60,
    "cooldown-5xx-secs": 15,
    "cooldown-network-secs": 10
  },
  "body_limit_mb": 10,
  "streaming": {
    "keepalive-seconds": 15,
    "bootstrap-retries": 1
  },
  "connect_timeout": 30,
  "request_timeout": 300,
  "claude_keys_count": 3,
  "openai_keys_count": 2,
  "gemini_keys_count": 1,
  "compat_keys_count": 1
}
```

**Source:** `crates/server/src/handler/admin.rs`

---

#### GET /admin/metrics

Same as `/metrics`. Returns full metrics snapshot.

**Source:** `crates/server/src/handler/admin.rs`

---

#### GET /admin/models

Lists all available models across all providers.

**Response:**
```json
{
  "models": [
    { "id": "gpt-4", "provider": "openai", "owned_by": "openai" },
    { "id": "claude-sonnet-4-20250514", "provider": "claude", "owned_by": "claude" }
  ]
}
```

**Source:** `crates/server/src/handler/admin.rs`

---

### Authenticated API routes

All routes below require a valid API key (see Authentication section).

These routes have a request body size limit configured by `body-limit-mb` (default: 10 MB).

---

#### GET /v1/models

Lists available models in OpenAI-compatible format.

**Response:**
```json
{
  "object": "list",
  "data": [
    {
      "id": "gpt-4",
      "object": "model",
      "created": 1740000000,
      "owned_by": "openai"
    },
    {
      "id": "claude-sonnet-4-20250514",
      "object": "model",
      "created": 1740000000,
      "owned_by": "claude"
    }
  ]
}
```

> Note: `created` is the current UTC timestamp (`chrono::Utc::now().timestamp()`), not a fixed value.

**Source:** `crates/server/src/handler/models.rs`

---

#### POST /v1/chat/completions

OpenAI Chat Completions API endpoint. Accepts OpenAI-format requests and routes to any provider (OpenAI, Claude, Gemini, OpenAI-compat) with automatic format translation.

**Source format:** `Format::OpenAI`
**Allowed formats:** all (auto-resolved from model name)

**Request body:** Standard OpenAI chat completions format with required `model` and `messages` fields. The `stream` field (boolean, default `false`) controls streaming.

**Dispatch flow:**
1. Parse `model`, `stream`, `User-Agent` from request
2. Route through `dispatch()` which resolves providers, picks credentials, translates, and executes

**Source:** `crates/server/src/handler/chat_completions.rs`

---

#### POST /v1/messages

Claude Messages API passthrough. Accepts Claude-format requests and routes only to Claude providers.

**Source format:** `Format::Claude`
**Allowed formats:** `[Format::Claude]` only

**Request body:** Standard Anthropic Messages API format with required `model` and `messages` fields.

**Source:** `crates/server/src/handler/messages.rs`

---

#### POST /v1/responses

OpenAI Responses API passthrough routed through the unified dispatch pipeline. Supports provider selection, retry/failover, auth-profile pinning, and streaming passthrough for OpenAI-family upstreams.

**Allowed formats:** `Format::OpenAI` only

**Behavior:** Parses `model`, optional `models[]`, `stream`, and optional `x-prism-auth-profile`, then dispatches with `responses_passthrough=true`. OpenAI-compatible upstreams receive the original Responses payload; Codex uses its native executor and payload normalization.

**Source:** `crates/server/src/handler/responses.rs`

---

#### GET /v1/responses/ws

WebSocket facade for the OpenAI Responses API. Accepts websocket JSON requests with `type: "response.create"` or `type: "response.append"` and forwards upstream Responses SSE events back as websocket text frames.

**Allowed formats:** `Format::OpenAI` only

**Behavior:** Uses the same auth, routing, and request logging stack as the HTTP Responses API. The gateway pins the successful credential after the first request. For Codex, subsequent websocket turns preserve `previous_response_id`; other upstreams fall back to transcript merge semantics. Completion is signaled by the terminal `response.completed` event; Prism does not synthesize a websocket `[DONE]` frame.

**Source:** `crates/server/src/handler/responses_ws.rs`

---

### Dashboard routes

Dashboard login is public; all other dashboard routes require dashboard auth via either `Authorization: Bearer <jwt>` or the HttpOnly `dashboard_session` cookie.

#### POST /api/dashboard/auth/login

Authenticates the dashboard user with bcrypt password verification, returns a JWT payload, and sets the same token as an HttpOnly session cookie for browser clients.

**Response:**
```json
{
  "token": "<jwt>",
  "expires_in": 3600,
  "token_type": "Bearer"
}
```

**Source:** `crates/server/src/handler/dashboard/auth.rs`

---

#### POST /api/dashboard/auth/refresh

Refreshes a valid dashboard JWT and returns a new token with the configured TTL.

**Source:** `crates/server/src/handler/dashboard/auth.rs`

---

#### GET /api/dashboard/providers

Lists providers with masked secrets and summarized auth profile state.

#### POST /api/dashboard/providers

Creates a logical provider family. `api_key` and `auth_profiles[]` are mutually exclusive, but both may be omitted so auth profiles can be attached later through the dedicated auth profile APIs.

#### GET /api/dashboard/providers/{name}

Returns the full provider definition with masked auth profile state.

#### PATCH /api/dashboard/providers/{name}

Updates shared provider settings and optionally replaces `auth_profiles[]`.

#### DELETE /api/dashboard/providers/{name}

Deletes a provider.

#### POST /api/dashboard/providers/fetch-models

Fetches a live model inventory from the upstream using the draft provider settings supplied in the request body. Intended for dashboard onboarding and registry workflows.

#### POST /api/dashboard/providers/{name}/health

Runs a live provider health probe and returns `{ provider, upstream, status, checked_at, latency_ms, checks[] }`.

#### POST /api/dashboard/providers/{name}/test-request

Sends a direct operator test request to the selected provider and returns the effective upstream request/response payloads. This is dashboard-only validation for provider health and UX, not a public gateway API.

#### POST /api/dashboard/providers/{name}/presentation-preview

Builds a presentation-layer preview for the selected provider and returns the effective headers, mutation trace, and final request body that would be sent upstream.

**Source:** `crates/server/src/handler/dashboard/providers/`

---

#### GET /api/dashboard/providers/capabilities

Returns dashboard runtime truth for every provider. Unlike the editable provider CRUD payload, this response is probe-oriented and includes provider identity, presentation, wire API, flattened model inventory, and the latest cached capability probe states.

**Response shape:**
```json
{
  "providers": [
    {
      "name": "codex-gateway",
      "format": "openai",
      "upstream": "codex",
      "upstream_protocol": "open_ai",
      "wire_api": "responses",
      "presentation_profile": "codex-cli",
      "presentation_mode": "always",
      "models": [
        { "id": "gpt-5", "alias": null }
      ],
      "probe_status": "warning",
      "checked_at": "2026-03-16T12:00:00Z",
      "probe": {
        "text": { "status": "verified", "message": null },
        "stream": { "status": "verified", "message": null },
        "tools": { "status": "verified", "message": null },
        "images": { "status": "verified", "message": null },
        "json_schema": { "status": "unknown", "message": "no live probe implemented" },
        "reasoning": { "status": "unknown", "message": "no live probe implemented" },
        "count_tokens": { "status": "unsupported", "message": "Codex backend does not expose count_tokens" }
      },
      "disabled": false
    }
  ]
}
```

**Notes:**
- `probe_status` is a provider-level summary derived from the cached probe result (`ok`, `warning`, `error`, `unknown`).
- `probe.*.status` is per-capability runtime truth (`verified`, `failed`, `unknown`, `unsupported`).
- `models[]` is the provider-local model mapping used by the dashboard model registry.

**Source:** `crates/server/src/handler/dashboard/control_plane.rs`

---

#### GET /api/dashboard/protocols/matrix

Returns the dashboard protocol control-plane payload. This endpoint is no longer a coarse three-column matrix; it exposes both the explicit route inventory and per-provider client-surface coverage.

**Response shape:**
```json
{
  "endpoints": [
    {
      "id": "openai_responses_ws",
      "family": "open_ai",
      "method": "GET",
      "path": "/v1/responses/ws",
      "description": "WebSocket facade over Responses SSE with create/append semantics.",
      "scope": "public",
      "transport": "web_socket",
      "operation": "generate",
      "stream_transport": "web_socket_events",
      "state": {
        "status": "verified",
        "message": "at least one active provider has verified runtime support"
      },
      "note": "Terminal completion is signaled by response.completed, not [DONE]."
    }
  ],
  "coverage": [
    {
      "provider": "codex-gateway",
      "format": "openai",
      "upstream": "codex",
      "upstream_protocol": "open_ai",
      "wire_api": "responses",
      "disabled": false,
      "surface_id": "openai_responses_ws",
      "surface_label": "OpenAI Responses WS",
      "ingress_protocol": "open_ai",
      "execution_mode": "native",
      "state": {
        "status": "verified",
        "message": null
      }
    }
  ]
}
```

**Notes:**
- `endpoints[]` is built from the current backend router contract and includes public routes, provider-scoped routes, non-generation operations, and WebSocket transports.
- `coverage[]` is calculated per provider and per client surface such as `openai_chat`, `openai_responses`, `openai_responses_ws`, `claude_messages`, `claude_count_tokens`, `gemini_generate`, and `gemini_stream`.
- `execution_mode` is omitted when a provider does not expose the surface at all. Otherwise it reflects whether Prism can serve that surface `native`ly or through protocol adaptation.

**Source:** `crates/server/src/handler/dashboard/control_plane.rs`

---

#### GET /api/dashboard/auth-profiles

Lists flattened auth profile state across all providers. This includes mode, header kind, masked secret or runtime access-token state, refresh-token presence, expiry, account metadata, and upstream presentation config.

#### GET /api/dashboard/auth-profiles/runtime

Returns managed-auth runtime truth for the dashboard: `{ storage_dir, codex_auth_file, proxy_url }`. The frontend uses this to show the actual runtime token directory, default local import path, and the server egress used for Codex auth exchange/device/refresh requests.

#### POST /api/dashboard/auth-profiles

Creates a new auth profile under an existing provider.

#### PUT /api/dashboard/auth-profiles/{provider}/{profile}

Replaces an existing auth profile in place. For static auth modes, omitting `secret` preserves the existing secret when the mode is unchanged.

#### DELETE /api/dashboard/auth-profiles/{provider}/{profile}

Deletes an auth profile and clears any persisted runtime OAuth state for that profile.

#### POST /api/dashboard/auth-profiles/codex/oauth/start

Starts a Codex OAuth PKCE flow and returns `{ state, auth_url, provider, profile_id, expires_in }`.

#### POST /api/dashboard/auth-profiles/codex/oauth/complete

Completes the OAuth code exchange, hydrates the auth profile, and persists runtime OAuth tokens into the managed auth runtime directory (`*.managed-auth.d/*.json`) rather than the YAML config. The token exchange uses `managed-auth.proxy-url` when configured.

#### POST /api/dashboard/auth-profiles/codex/device/start

Starts a Codex device authorization flow and returns `{ state, verification_url, user_code, interval_secs, expires_in }`. The device flow request uses `managed-auth.proxy-url` when configured.

#### POST /api/dashboard/auth-profiles/codex/device/poll

Polls a pending device authorization session. Returns `{ status: "pending" }` while waiting and `{ status: "completed", profile }` after the token exchange succeeds. The token exchange step uses `managed-auth.proxy-url` when configured.

#### POST /api/dashboard/auth-profiles/{provider}/{profile}/connect

Connects a managed auth profile that expects operator-supplied runtime credentials. Prism currently supports `anthropic-claude-subscription` here by accepting a Claude setup-token, validating the provider/base URL constraints, and storing the token only in the managed auth runtime directory.

#### POST /api/dashboard/auth-profiles/{provider}/{profile}/import-local

Imports a server-local Codex CLI auth bundle into the managed auth runtime directory. The request body may optionally provide `{ "path": "/abs/path/auth.json" }`; when omitted, Prism falls back to `managed-auth.codex-auth-file`, then `PRISM_CODEX_AUTH_FILE`, then `~/.codex/auth.json`. This endpoint only supports `codex-oauth` profiles.

#### POST /api/dashboard/auth-profiles/{provider}/{profile}/refresh

Refreshes an existing refreshable managed auth profile. Prism currently supports `codex-oauth` here and persists the updated runtime tokens into the managed auth runtime directory. The refresh request uses `managed-auth.proxy-url` when configured.

**Source:** `crates/server/src/handler/dashboard/auth_profiles.rs`

---

## Authentication

**Source:** `crates/server/src/auth.rs`

Authentication is enforced on API routes (`/v1/*`) via the `auth_middleware`.

### Token extraction

The middleware checks two header locations in order:

1. `Authorization: Bearer <token>` -- standard Bearer token
2. `x-api-key: <token>` -- alternative header (Anthropic convention)

### Behavior

- If `config.auth_keys` is empty (no keys configured), auth is skipped entirely -- all requests pass through.
- If keys are configured, the extracted token is looked up in `AuthKeyStore` (O(1) HashMap lookup).
- Expired keys return `ProxyError::KeyExpired` (401).
- Invalid keys return `ProxyError::Auth("Invalid API key")` (401).
- On success, the middleware injects `api_key_id`, `tenant_id`, and `auth_key` into `RequestContext`.

### Example

```bash
# Using Bearer token
curl -H "Authorization: Bearer your-proxy-key" \
  http://localhost:8317/v1/chat/completions

# Using x-api-key header
curl -H "x-api-key: your-proxy-key" \
  http://localhost:8317/v1/chat/completions
```

---

## Middleware Stack

Middleware is applied in layers. Axum evaluates outer layers first on the request path and last on the response path.

**Source:** `crates/server/src/lib.rs` (`build_router()`)

### Application order (outer to inner)

```
Request flow:
  TraceLayer (tower-http)
    -> CorsLayer (permissive)
      -> request_context_middleware (injects RequestContext)
        -> request_logging_middleware (logs request/response)
          -> [for API routes only] auth_middleware
            -> [for API routes only] RequestBodyLimitLayer
              -> Handler
```

| Layer | Scope | Description |
|-------|-------|-------------|
| `TraceLayer` | Global | tower-http tracing integration. |
| `CorsLayer::permissive()` | Global | Permissive CORS (all origins, methods, headers). |
| `request_context_middleware` | Global | Injects `RequestContext` extension with `request_id` (UUID), `start_time`, and `client_ip` (from `X-Forwarded-For` or `X-Real-IP`). |
| `request_logging_middleware` | Global | Logs request method/path on entry and status/elapsed_ms on completion using `tracing`. |
| `auth_middleware` | API routes only | Validates Bearer token or x-api-key header against configured keys. |
| `RequestBodyLimitLayer` | API routes only | Enforces `body_limit_mb` (default 10 MB) on request bodies. |

---

## Error Response Format

All errors are returned as JSON with the appropriate HTTP status code.

### Standard format

```json
{
  "error": {
    "message": "human-readable error description",
    "type": "error_type_category",
    "code": "machine_readable_code"
  }
}
```

See [errors.md](types/errors.md) for the full mapping of error variants to types, codes, and HTTP status codes.

### Upstream passthrough

When the upstream provider returns an error with a valid JSON body, that body is passed through verbatim with the upstream's HTTP status code, preserving provider-specific error details.

---

## AppState

Shared application state injected into all handlers via axum's `State` extractor.

**Source:** `crates/server/src/lib.rs`

```rust
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<ArcSwap<Config>>,
    pub router: Arc<CredentialRouter>,
    pub executors: Arc<ExecutorRegistry>,
    pub translators: Arc<TranslatorRegistry>,
    pub metrics: Arc<Metrics>,
    pub log_store: Arc<dyn LogStore>,
    pub config_path: Arc<Mutex<String>>,
    pub rate_limiter: Arc<CompositeRateLimiter>,
    pub cost_calculator: Arc<CostCalculator>,
    pub response_cache: Option<Arc<dyn ResponseCacheBackend>>,
    pub http_client_pool: Arc<HttpClientPool>,
    pub thinking_cache: Option<Arc<ThinkingCache>>,
    pub start_time: Instant,
    pub login_limiter: Arc<LoginRateLimiter>,
    pub catalog: Arc<ProviderCatalog>,
    pub health_manager: Arc<HealthManager>,
    pub auth_runtime: Arc<AuthRuntimeManager>,
    pub oauth_sessions: Arc<DashMap<String, PendingCodexOauthSession>>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `config` | `Arc<ArcSwap<Config>>` | Hot-reloadable configuration. |
| `router` | `Arc<CredentialRouter>` | Credential selection and routing. |
| `executors` | `Arc<ExecutorRegistry>` | Provider executor instances. |
| `translators` | `Arc<TranslatorRegistry>` | Format translation functions. |
| `metrics` | `Arc<Metrics>` | In-memory metrics counters. |
| `log_store` | `Arc<dyn LogStore>` | Dashboard request log backend. |
| `config_path` | `Arc<Mutex<String>>` | Path to config file (for hot-reload). |
| `rate_limiter` | `Arc<CompositeRateLimiter>` | Per-key and global rate limiter. |
| `cost_calculator` | `Arc<CostCalculator>` | Token cost calculation. |
| `response_cache` | `Option<Arc<dyn ResponseCacheBackend>>` | Optional response cache (Moka). |
| `http_client_pool` | `Arc<HttpClientPool>` | Shared outbound HTTP client pool. |
| `thinking_cache` | `Option<Arc<ThinkingCache>>` | Optional reasoning/thinking cache. |
| `start_time` | `Instant` | Server start time (for uptime calculation). |
| `login_limiter` | `Arc<LoginRateLimiter>` | Dashboard login brute-force protection. |
| `catalog` | `Arc<ProviderCatalog>` | Provider inventory snapshot for dashboard/control plane. |
| `health_manager` | `Arc<HealthManager>` | Runtime provider health and outlier state. |
| `auth_runtime` | `Arc<AuthRuntimeManager>` | Runtime OAuth/PCKE helper and token refresher. |
| `oauth_sessions` | `Arc<DashMap<...>>` | Pending dashboard OAuth sessions keyed by `state`. |
