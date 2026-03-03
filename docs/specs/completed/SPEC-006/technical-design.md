# Technical Design: Security & Authentication

| Field     | Value                       |
|-----------|-----------------------------|
| Spec ID   | SPEC-006                    |
| Title     | Security & Authentication   |
| Author    | Prism Team               |
| Status    | Completed                   |
| Created   | 2026-02-27                  |
| Updated   | 2026-02-27                  |

## Overview

Security is implemented through layered middleware: authentication checks client credentials, TLS encrypts transport, CORS allows cross-origin access, and body limits prevent abuse. The middleware stack is composed in `build_router()` with careful ordering to ensure auth applies only to API routes while admin and health endpoints remain open. See PRD (SPEC-006) for requirements.

## Backend Implementation

### Module Structure

```
crates/server/src/auth.rs      -- auth_middleware function
crates/server/src/lib.rs       -- build_router(), middleware stack, AppState
crates/core/src/config.rs      -- TlsConfig, api_keys, body_limit_mb, passthrough_headers
prism/src/main.rs           -- TLS server setup with Rustls
```

### auth_middleware

```rust
pub async fn auth_middleware(
    State(state): State<AppState>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, ProxyError> {
    let config = state.config.load();

    // If no API keys configured, skip auth (open proxy mode)
    if config.api_keys.is_empty() {
        return Ok(next.run(request).await);
    }

    // Extract token: try Authorization: Bearer first, fall back to x-api-key header
    let token = request.headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .or_else(|| {
            request.headers()
                .get("x-api-key")
                .and_then(|v| v.to_str().ok())
        });

    match token {
        Some(t) if config.api_keys_set.contains(t) => Ok(next.run(request).await),
        _ => Err(ProxyError::Auth("Invalid API key".to_string())),
    }
}
```

**Key behaviors:**

1. **Open mode:** If `api_keys` is empty, all requests pass through without auth
2. **Token extraction priority:** `Authorization: Bearer <token>` is checked first, then `x-api-key: <token>` header
3. **O(1) lookup:** Token is checked against `api_keys_set` (HashSet), built during config sanitization
4. **Hot-reload aware:** Reads config via `state.config.load()` (ArcSwap) on every request, so key changes take effect immediately

### TLS Configuration

#### TlsConfig

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case", default)]
pub struct TlsConfig {
    pub enable: bool,
    pub cert: Option<String>,  // path to PEM certificate file
    pub key: Option<String>,   // path to PEM private key file
}
```

#### Rustls Integration (main.rs)

When `cfg.tls.enable` is true:

1. Load certificates from PEM file via `CertificateDer::pem_file_iter`
2. Load private key from PEM file via `PrivateKeyDer::from_pem_file`
3. Build `rustls::ServerConfig` with `with_no_client_auth().with_single_cert(certs, key)`
4. Create `tokio_rustls::TlsAcceptor`
5. Accept TCP connections, perform TLS handshake, then serve via hyper
6. Graceful shutdown with `tokio::select!` on shutdown signal

When TLS is disabled, the server runs plain HTTP via `axum::serve`.

### CORS

```rust
.layer(CorsLayer::permissive())
```

- Uses `tower_http::cors::CorsLayer::permissive()`
- Allows all origins, all methods, all headers
- Applied as an outer layer on the entire router (covers all routes)
- Rationale: API gateway intended for trusted application clients; permissive CORS simplifies integration

### Request Body Size Limit

```rust
.layer(RequestBodyLimitLayer::new(body_limit_bytes))
```

- Applied only to API routes (not admin or health endpoints)
- `body_limit_bytes = config.body_limit_mb * 1024 * 1024`
- Default: 10 MB
- Rejects oversized requests at the transport layer before full buffering

### Middleware Ordering in Router

```rust
pub fn build_router(state: AppState) -> Router {
    // 1. Public routes (no auth)
    let public_routes = Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics));

    // 2. Admin routes (no auth, read-only)
    let admin_routes = Router::new()
        .route("/admin/config", get(admin_config))
        .route("/admin/metrics", get(admin_metrics))
        .route("/admin/models", get(admin_models));

    // 3. API routes (auth required, body limit)
    let api_routes = Router::new()
        .route("/v1/models", get(list_models))
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/messages", post(messages))
        .route("/v1/responses", post(responses))
        .layer(RequestBodyLimitLayer::new(body_limit_bytes))  // inner: body limit
        .layer(from_fn_with_state(state, auth_middleware));    // outer: auth check

    // 4. Compose all route groups, then add global middleware
    Router::new()
        .merge(public_routes)
        .merge(admin_routes)
        .merge(api_routes)
        .layer(from_fn(request_logging_middleware))
        .layer(from_fn(request_context_middleware))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
```

**Layer evaluation order (outermost to innermost):**
1. `TraceLayer` -- HTTP tracing
2. `CorsLayer` -- CORS headers
3. `request_context_middleware` -- request ID / context
4. `request_logging_middleware` -- request/response logging
5. (API routes only) `auth_middleware` -- authentication
6. (API routes only) `RequestBodyLimitLayer` -- body size limit
7. Handler

### Admin Endpoints

Admin endpoints are merged without the auth middleware layer:

- `GET /admin/config` -- returns sanitized config (secrets removed)
- `GET /admin/metrics` -- returns runtime metrics
- `GET /admin/models` -- returns available models

These are read-only and intended for monitoring/debugging.

### Passthrough Headers

```rust
// In dispatch.rs, after successful upstream response:
for header_name in &config.passthrough_headers {
    if let Some(val) = response.headers.get(header_name) {
        builder = builder.header(header_name.as_str(), val.as_str());
    }
}
```

- Configured via `passthrough_headers` in config (list of header names)
- Forwards specified upstream response headers to the client
- Common use: forwarding rate limit headers (`x-ratelimit-remaining`, etc.)

## Configuration Changes

```yaml
# Authentication
api-keys:
  - "sk-proxy-key-1"
  - "sk-proxy-key-2"

# TLS
tls:
  enable: false
  cert: "/path/to/cert.pem"
  key: "/path/to/key.pem"

# Body size limit
body-limit-mb: 10

# Response header forwarding
passthrough-headers:
  - "x-ratelimit-remaining"
  - "x-ratelimit-limit"
```

## Provider Compatibility

| Provider | Supported | Notes |
|----------|-----------|-------|
| OpenAI   | Yes       | Clients use `Authorization: Bearer` |
| Claude   | Yes       | Clients use `x-api-key` header |
| Gemini   | Yes       | Clients use `Authorization: Bearer` |
| Compat   | Yes       | Follows OpenAI convention |

## Test Strategy

- **Unit tests:** Auth middleware tested with valid/invalid/missing tokens
- **Integration tests:** Verify 401 on bad key, 200 on good key, open mode with empty api_keys
- **Manual verification:** Test TLS with `curl --cacert`, verify CORS headers in browser dev tools
