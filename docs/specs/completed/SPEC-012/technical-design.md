# Technical Design: Rate Limiting

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-012       |
| Title     | Rate Limiting  |
| Author    | AI Proxy Team  |
| Status    | Completed      |
| Created   | 2026-03-01     |
| Updated   | 2026-03-01     |

## Overview

Sliding-window rate limiter that enforces per-API-key and global RPM limits, returning standard `x-ratelimit-*` response headers and HTTP 429 when limits are exceeded. Supports hot-reload via config watcher.

Reference: [SPEC-012 PRD](prd.md)

## API Design

### Response Headers (on every proxied request)

```
x-ratelimit-limit: 60
x-ratelimit-remaining: 59
x-ratelimit-reset: 45
```

### Rate Limited Response

```http
HTTP/1.1 429 Too Many Requests
retry-after: 60

{
  "error": {
    "message": "rate limit exceeded: Rate limit exceeded. Retry after 45s",
    "type": "rate_limit_error",
    "code": "rate_limit_exceeded"
  }
}
```

## Backend Implementation

### Module Structure

```
crates/core/src/
├── config.rs          # RateLimitConfig
├── error.rs           # ProxyError::RateLimited
└── rate_limit.rs      # RateLimiter, SlidingWindow, RateLimitInfo

crates/server/src/
├── lib.rs             # AppState.rate_limiter
└── middleware/
    └── rate_limit.rs  # rate_limit_middleware

src/
└── app.rs             # Initialization + hot-reload wiring
```

### Key Types

```rust
// crates/core/src/config.rs
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct RateLimitConfig {
    pub enabled: bool,
    pub global_rpm: u32,
    pub per_key_rpm: u32,
}

// crates/core/src/rate_limit.rs
pub struct RateLimiter {
    global: Mutex<SlidingWindow>,
    per_key: RwLock<HashMap<String, Mutex<SlidingWindow>>>,
    config: RwLock<RateLimitConfig>,
}

pub struct RateLimitInfo {
    pub allowed: bool,
    pub remaining: u32,
    pub limit: u32,
    pub reset_secs: u64,
}

// crates/core/src/error.rs
pub enum ProxyError {
    #[error("rate limit exceeded: {0}")]
    RateLimited(String),  // → HTTP 429 + Retry-After
}
```

### Flow

1. Request arrives at `rate_limit_middleware`
2. Extract API key from `Authorization: Bearer` or `x-api-key` header
3. `RateLimiter::check(api_key)` — prune expired timestamps, check global + per-key limits
4. If `!allowed` → return `ProxyError::RateLimited` (429)
5. `RateLimiter::record(api_key)` — add timestamp to sliding window
6. Execute request via `next.run()`
7. Inject `x-ratelimit-*` headers into response

### Algorithm

- **60-second sliding window**: Timestamps older than 60s are pruned on each check/record
- **Most restrictive wins**: When both global and per-key limits are active, the lower remaining count is used
- **Concurrency**: `Mutex<SlidingWindow>` for atomic operations, `RwLock<HashMap>` for per-key map

## Configuration Changes

```yaml
rate-limit:
  enabled: true
  global-rpm: 60          # Global requests per minute (0 = unlimited)
  per-key-rpm: 30         # Per-API-key requests per minute (0 = unlimited)
```

## Provider Compatibility

| Provider | Supported | Notes |
|----------|-----------|-------|
| OpenAI   | Yes       | Applied at proxy layer before routing |
| Claude   | Yes       | Same middleware |
| Gemini   | Yes       | Same middleware |
| OpenAI-compat | Yes  | Same middleware |

## Task Breakdown

- [x] T1: `RateLimitConfig` struct in `config.rs` with defaults
- [x] T2: `RateLimiter` core algorithm (sliding window) in `rate_limit.rs`
- [x] T3: `RateLimited` error variant with 429 + `Retry-After`
- [x] T4: `rate_limit_middleware` in `crates/server/src/middleware/`
- [x] T5: `AppState` integration + hot-reload wiring in `app.rs`
- [x] T6: Unit tests (disabled, global RPM, per-key RPM, remaining count, config update)
- [x] T7: `config.example.yaml` documentation

## Test Strategy

- **Unit tests:** `rate_limit.rs` — disabled mode, global RPM enforcement, per-key isolation, remaining count, hot-reload
- **Integration tests:** Dashboard test verifies rate limit config via API
- **Manual verification:** Start server with `rate-limit.enabled: true`, send burst requests, verify 429 responses
