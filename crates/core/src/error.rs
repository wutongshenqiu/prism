use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// Unified error type for all proxy operations.
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
        /// Parsed from upstream `Retry-After` header (seconds), if present.
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

    #[error("rate limit exceeded: {message}")]
    RateLimited {
        message: String,
        /// Seconds until the rate limit resets.
        retry_after_secs: u64,
    },

    #[error("model access denied: {0}")]
    ModelNotAllowed(String),

    #[error("API key expired")]
    KeyExpired,

    #[error("internal error: {0}")]
    Internal(String),
}

impl ProxyError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::Config(_) | Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Auth(_) | Self::KeyExpired => StatusCode::UNAUTHORIZED,
            Self::ModelNotAllowed(_) => StatusCode::FORBIDDEN,
            Self::NoCredentials { .. } => StatusCode::SERVICE_UNAVAILABLE,
            Self::ModelCooldown { .. } | Self::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
            Self::Upstream { status, .. } => {
                StatusCode::from_u16(*status).unwrap_or(StatusCode::BAD_GATEWAY)
            }
            Self::Network(_) => StatusCode::BAD_GATEWAY,
            Self::Translation(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::ModelNotFound(_) => StatusCode::NOT_FOUND,
        }
    }

    fn error_type(&self) -> &str {
        match self {
            Self::Auth(_) | Self::KeyExpired => "authentication_error",
            Self::ModelNotAllowed(_) => "permission_error",
            Self::NoCredentials { .. } => "insufficient_quota",
            Self::ModelCooldown { .. } | Self::RateLimited { .. } => "rate_limit_error",
            Self::BadRequest(_) => "invalid_request_error",
            Self::ModelNotFound(_) => "invalid_request_error",
            Self::Upstream { .. } => "upstream_error",
            _ => "server_error",
        }
    }

    fn error_code(&self) -> &str {
        match self {
            Self::Auth(_) | Self::KeyExpired => "invalid_api_key",
            Self::ModelNotAllowed(_) => "model_not_allowed",
            Self::NoCredentials { .. } => "insufficient_quota",
            Self::ModelCooldown { .. } | Self::RateLimited { .. } => "rate_limit_exceeded",
            Self::ModelNotFound(_) => "model_not_found",
            Self::BadRequest(_) => "invalid_request",
            _ => "internal_error",
        }
    }
}

impl IntoResponse for ProxyError {
    fn into_response(self) -> Response {
        let status = self.status_code();

        // For upstream errors, try to pass through the original JSON body
        if let Self::Upstream { body, .. } = &self
            && serde_json::from_str::<serde_json::Value>(body).is_ok()
        {
            return (status, [("content-type", "application/json")], body.clone()).into_response();
        }

        let body = json!({
            "error": {
                "message": self.to_string(),
                "type": self.error_type(),
                "code": self.error_code(),
            }
        });

        let mut response = (
            status,
            [("content-type", "application/json")],
            body.to_string(),
        )
            .into_response();

        // Add Retry-After header for rate limited responses
        let retry_secs = match &self {
            Self::RateLimited {
                retry_after_secs, ..
            } => Some(*retry_after_secs),
            Self::ModelCooldown { seconds, .. } => Some(*seconds),
            _ => None,
        };
        if let Some(secs) = retry_secs
            && let Ok(val) = secs.to_string().parse()
        {
            response.headers_mut().insert("retry-after", val);
        }

        response
    }
}

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

impl From<serde_json::Error> for ProxyError {
    fn from(e: serde_json::Error) -> Self {
        Self::Translation(format!("JSON error: {e}"))
    }
}
