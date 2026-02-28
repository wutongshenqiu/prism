use crate::AppState;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{extract::State, http::Request, middleware::Next, response::Response};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
}

/// JWT authentication middleware for dashboard endpoints.
pub async fn dashboard_auth_middleware(
    State(state): State<AppState>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, Response> {
    let config = state.config.load();
    let secret = config.dashboard.resolve_jwt_secret().ok_or_else(|| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "jwt_not_configured",
            "Dashboard JWT secret not configured",
        )
    })?;

    // Extract token from Authorization: Bearer header or query param
    let token = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string())
        .or_else(|| {
            request.uri().query().and_then(|q| {
                q.split('&')
                    .find_map(|p| p.strip_prefix("token=").map(|t| t.to_string()))
            })
        });

    let token = token.ok_or_else(|| {
        error_response(
            StatusCode::UNAUTHORIZED,
            "missing_token",
            "Authorization header required",
        )
    })?;

    let key = DecodingKey::from_secret(secret.as_bytes());
    let token_data = decode::<Claims>(&token, &key, &Validation::default()).map_err(|e| {
        let (code, msg) = match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                ("token_expired", "Token has expired")
            }
            _ => ("invalid_token", "Invalid token"),
        };
        error_response(StatusCode::UNAUTHORIZED, code, msg)
    })?;

    // Inject claims as extension
    let mut request = request;
    request.extensions_mut().insert(token_data.claims);

    Ok(next.run(request).await)
}

/// Generate a JWT token for a user.
pub fn generate_token(
    username: &str,
    secret: &str,
    ttl_secs: u64,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = chrono::Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: username.to_string(),
        iat: now,
        exp: now + ttl_secs as usize,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}

fn error_response(status: StatusCode, code: &str, message: &str) -> Response {
    let body = json!({
        "error": code,
        "message": message,
    });
    (
        status,
        [("content-type", "application/json")],
        body.to_string(),
    )
        .into_response()
}
