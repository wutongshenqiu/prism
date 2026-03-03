# Error Types Reference

**Source:** `crates/core/src/error.rs`

---

## ProxyError

Unified error type for all proxy operations. Implements `thiserror::Error`, `IntoResponse`, and conversions from `reqwest::Error` and `serde_json::Error`.

```rust
#[derive(Debug, thiserror::Error)]
pub enum ProxyError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("authentication failed: {0}")]
    Auth(String),

    #[error("no credentials available for provider {provider}, model {model}")]
    NoCredentials { provider: String, model: String },

    #[error("model {model} is in cooldown for {seconds}s")]
    ModelCooldown { model: String, seconds: u64 },

    #[error("upstream error (status {status}): {body}")]
    Upstream {
        status: u16,
        body: String,
        retry_after_secs: Option<u64>,
    },

    #[error("network error: {0}")]
    Network(String),

    #[error("translation error: {0}")]
    Translation(String),

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("model not found: {0}")]
    ModelNotFound(String),

    #[error("rate limit exceeded: {0}")]
    RateLimited(String),

    #[error("model access denied: {0}")]
    ModelNotAllowed(String),

    #[error("API key expired")]
    KeyExpired,

    #[error("internal error: {0}")]
    Internal(String),
}
```

---

## Variant details

| Variant | Fields | Description |
|---------|--------|-------------|
| `Config` | `String` | Configuration validation or loading error. |
| `Auth` | `String` | Client authentication failure (invalid or missing API key). |
| `NoCredentials` | `provider: String, model: String` | No available credentials for the given provider and model. All credentials may be disabled, circuit-broken, or not configured. |
| `ModelCooldown` | `model: String, seconds: u64` | The requested model's credentials are all in cooldown. |
| `Upstream` | `status: u16, body: String, retry_after_secs: Option<u64>` | Error response from upstream provider. `retry_after_secs` is parsed from the upstream `Retry-After` header if present. |
| `Network` | `String` | Network-level failure (timeout, connection refused, DNS failure). |
| `Translation` | `String` | Error during request/response format translation (JSON parse errors, missing fields). |
| `BadRequest` | `String` | Malformed client request (missing model field, invalid JSON, etc.). |
| `ModelNotFound` | `String` | No provider has a credential that supports the requested model. |
| `RateLimited` | `String` | Global or per-key rate limit exceeded (RPM, TPM, or daily cost). |
| `ModelNotAllowed` | `String` | The auth key does not have access to the requested model (restricted by `allowed_models`). |
| `KeyExpired` | (none) | The client's API key has passed its `expires_at` date. |
| `Internal` | `String` | Unexpected internal error (response build failure, task panic, etc.). |

---

## HTTP Status Code Mapping

The `status_code()` method maps each variant to an HTTP status code:

```rust
impl ProxyError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::Config(_) | Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,  // 500
            Self::Auth(_) | Self::KeyExpired => StatusCode::UNAUTHORIZED,               // 401
            Self::ModelNotAllowed(_) => StatusCode::FORBIDDEN,                          // 403
            Self::NoCredentials { .. } => StatusCode::SERVICE_UNAVAILABLE,              // 503
            Self::ModelCooldown { .. } | Self::RateLimited(_) => StatusCode::TOO_MANY_REQUESTS, // 429
            Self::Upstream { status, .. } => {
                StatusCode::from_u16(*status).unwrap_or(StatusCode::BAD_GATEWAY)       // pass-through or 502
            }
            Self::Network(_) => StatusCode::BAD_GATEWAY,                               // 502
            Self::Translation(_) => StatusCode::INTERNAL_SERVER_ERROR,                  // 500
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,                             // 400
            Self::ModelNotFound(_) => StatusCode::NOT_FOUND,                            // 404
        }
    }
}
```

### Summary table

| Variant | HTTP Status | Code |
|---------|-------------|------|
| `Config` | 500 Internal Server Error | |
| `Auth` | 401 Unauthorized | |
| `KeyExpired` | 401 Unauthorized | |
| `ModelNotAllowed` | 403 Forbidden | |
| `NoCredentials` | 503 Service Unavailable | |
| `ModelCooldown` | 429 Too Many Requests | |
| `RateLimited` | 429 Too Many Requests | |
| `Upstream` | pass-through (e.g., 429, 500) or 502 | |
| `Network` | 502 Bad Gateway | |
| `Translation` | 500 Internal Server Error | |
| `BadRequest` | 400 Bad Request | |
| `ModelNotFound` | 404 Not Found | |
| `Internal` | 500 Internal Server Error | |

---

## Error Type and Code Helpers

The `error_type()` and `error_code()` are **private** helper methods (not part of the public API) used internally by the `IntoResponse` implementation to produce structured error classification for the JSON response body:

### error_type()

| Variant | error_type |
|---------|------------|
| `Auth`, `KeyExpired` | `"authentication_error"` |
| `ModelNotAllowed` | `"permission_error"` |
| `NoCredentials` | `"insufficient_quota"` |
| `ModelCooldown`, `RateLimited` | `"rate_limit_error"` |
| `BadRequest` | `"invalid_request_error"` |
| `ModelNotFound` | `"invalid_request_error"` |
| `Upstream` | `"upstream_error"` |
| all others | `"server_error"` |

### error_code()

| Variant | error_code |
|---------|------------|
| `Auth`, `KeyExpired` | `"invalid_api_key"` |
| `ModelNotAllowed` | `"model_not_allowed"` |
| `NoCredentials` | `"insufficient_quota"` |
| `ModelCooldown`, `RateLimited` | `"rate_limit_exceeded"` |
| `ModelNotFound` | `"model_not_found"` |
| `BadRequest` | `"invalid_request"` |
| all others | `"internal_error"` |

---

## Error Response JSON Format

The `IntoResponse` implementation produces JSON error responses.

### Standard error response

For all variants except `Upstream` (with valid JSON body):

```json
{
  "error": {
    "message": "authentication failed: Invalid API key",
    "type": "authentication_error",
    "code": "invalid_api_key"
  }
}
```

### Upstream passthrough

For `Upstream` errors where the body is valid JSON, the original upstream error body is passed through verbatim with the upstream's HTTP status code. This preserves provider-specific error details.

If the upstream body is not valid JSON, the standard error format is used instead.

### Retry-After header

For `RateLimited` and `ModelCooldown` responses, the `Retry-After` header is automatically set to `"60"` seconds.

---

## Automatic Conversions

### From `reqwest::Error`

```rust
impl From<reqwest::Error> for ProxyError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() {
            Self::Network(format!("request timed out: {e}"))
        } else if e.is_connect() {
            Self::Network(format!("connection failed: {e}"))
        } else {
            Self::Network(e.to_string())
        }
    }
}
```

### From `serde_json::Error`

```rust
impl From<serde_json::Error> for ProxyError {
    fn from(e: serde_json::Error) -> Self {
        Self::Translation(format!("JSON error: {e}"))
    }
}
```
