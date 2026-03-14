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

    // Enforce localhost-only access
    if config.dashboard.localhost_only {
        let client_ip = request
            .extensions()
            .get::<prism_core::context::RequestContext>()
            .and_then(|ctx| ctx.client_ip.clone());
        let is_local = client_ip
            .as_deref()
            .is_some_and(|ip| ip == "127.0.0.1" || ip == "::1" || ip == "localhost");
        if !is_local {
            tracing::warn!(
                client_ip = client_ip.as_deref().unwrap_or("unknown"),
                path = %request.uri().path(),
                "Dashboard access denied: non-localhost IP"
            );
            return Err(error_response(
                StatusCode::FORBIDDEN,
                "access_denied",
                "Dashboard access restricted to localhost",
            ));
        }
    }

    let secret = config.dashboard.resolve_jwt_secret().ok_or_else(|| {
        tracing::error!("Dashboard JWT secret not configured");
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "jwt_not_configured",
            "Dashboard JWT secret not configured",
        )
    })?;

    // Extract token from Authorization: Bearer header only
    let token = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string());

    let token = token.ok_or_else(|| {
        tracing::warn!(
            path = %request.uri().path(),
            "Dashboard auth failed: missing Authorization header"
        );
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
                tracing::debug!(path = %request.uri().path(), "Dashboard auth: token expired");
                ("token_expired", "Token has expired")
            }
            _ => {
                tracing::warn!(path = %request.uri().path(), "Dashboard auth failed: invalid token");
                ("invalid_token", "Invalid token")
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_token_valid() {
        let token = generate_token("admin", "test-secret", 3600).unwrap();
        assert!(!token.is_empty());

        // Decode and verify claims
        let key = DecodingKey::from_secret(b"test-secret");
        let data = decode::<Claims>(&token, &key, &Validation::default()).unwrap();
        assert_eq!(data.claims.sub, "admin");
        assert!(data.claims.exp > data.claims.iat);
        assert_eq!(data.claims.exp - data.claims.iat, 3600);
    }

    #[test]
    fn test_generate_token_different_users() {
        let t1 = generate_token("alice", "secret", 60).unwrap();
        let t2 = generate_token("bob", "secret", 60).unwrap();
        assert_ne!(t1, t2);

        let key = DecodingKey::from_secret(b"secret");
        let c1 = decode::<Claims>(&t1, &key, &Validation::default())
            .unwrap()
            .claims;
        let c2 = decode::<Claims>(&t2, &key, &Validation::default())
            .unwrap()
            .claims;
        assert_eq!(c1.sub, "alice");
        assert_eq!(c2.sub, "bob");
    }

    #[test]
    fn test_generate_token_wrong_secret_fails() {
        let token = generate_token("admin", "real-secret", 3600).unwrap();
        let key = DecodingKey::from_secret(b"wrong-secret");
        let result = decode::<Claims>(&token, &key, &Validation::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_token_expired() {
        // Generate token with 0 TTL (already expired)
        let now = chrono::Utc::now().timestamp() as usize;
        let claims = Claims {
            sub: "admin".to_string(),
            iat: now - 7200,
            exp: now - 3600, // expired 1h ago
        };
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(b"secret"),
        )
        .unwrap();

        let key = DecodingKey::from_secret(b"secret");
        let result = decode::<Claims>(&token, &key, &Validation::default());
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err().kind(),
            jsonwebtoken::errors::ErrorKind::ExpiredSignature
        ));
    }

    #[test]
    fn test_claims_serialization() {
        let claims = Claims {
            sub: "test-user".to_string(),
            iat: 1000,
            exp: 2000,
        };
        let json = serde_json::to_value(&claims).unwrap();
        assert_eq!(json["sub"], "test-user");
        assert_eq!(json["iat"], 1000);
        assert_eq!(json["exp"], 2000);

        // Roundtrip
        let deserialized: Claims = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized.sub, "test-user");
    }
}
