use crate::AppState;
use crate::middleware::dashboard_auth::{Claims, generate_token};
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// POST /api/dashboard/auth/login
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> impl IntoResponse {
    let config = state.config.load();
    let dashboard = &config.dashboard;

    if !dashboard.enabled {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "not_found", "message": "Dashboard is not enabled"})),
        );
    }

    // Verify username
    if body.username != dashboard.username {
        return (
            StatusCode::UNAUTHORIZED,
            Json(
                json!({"error": "invalid_credentials", "message": "Invalid username or password"}),
            ),
        );
    }

    // Verify password against bcrypt hash
    if dashboard.password_hash.is_empty()
        || !bcrypt::verify(&body.password, &dashboard.password_hash).unwrap_or(false)
    {
        return (
            StatusCode::UNAUTHORIZED,
            Json(
                json!({"error": "invalid_credentials", "message": "Invalid username or password"}),
            ),
        );
    }

    let secret = match dashboard.resolve_jwt_secret() {
        Some(s) => s,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "config_error", "message": "JWT secret not configured"})),
            );
        }
    };

    match generate_token(&body.username, &secret, dashboard.jwt_ttl_secs) {
        Ok(token) => (
            StatusCode::OK,
            Json(json!({
                "token": token,
                "expires_in": dashboard.jwt_ttl_secs,
                "token_type": "Bearer",
            })),
        ),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "token_error", "message": "Failed to generate token"})),
        ),
    }
}

/// POST /api/dashboard/auth/refresh
pub async fn refresh(
    State(state): State<AppState>,
    claims: axum::Extension<Claims>,
) -> impl IntoResponse {
    let config = state.config.load();
    let dashboard = &config.dashboard;

    let secret = match dashboard.resolve_jwt_secret() {
        Some(s) => s,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "config_error", "message": "JWT secret not configured"})),
            );
        }
    };

    match generate_token(&claims.sub, &secret, dashboard.jwt_ttl_secs) {
        Ok(token) => (
            StatusCode::OK,
            Json(json!({
                "token": token,
                "expires_in": dashboard.jwt_ttl_secs,
                "token_type": "Bearer",
            })),
        ),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "token_error", "message": "Failed to generate token"})),
        ),
    }
}
