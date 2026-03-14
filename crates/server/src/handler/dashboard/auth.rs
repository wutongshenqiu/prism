use crate::AppState;
use crate::middleware::dashboard_auth::{Claims, generate_token};
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

/// Tracks login attempts per IP for brute-force protection.
pub struct LoginRateLimiter {
    /// Map of IP → list of attempt timestamps within the lockout window.
    attempts: Mutex<HashMap<String, Vec<Instant>>>,
}

impl Default for LoginRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl LoginRateLimiter {
    pub fn new() -> Self {
        Self {
            attempts: Mutex::new(HashMap::new()),
        }
    }

    /// Record a failed login attempt. Returns true if the IP is now locked out.
    pub fn record_failure(&self, ip: &str, max_attempts: u32, window_secs: u64) -> bool {
        let mut map = self.attempts.lock().unwrap();
        let now = Instant::now();
        let cutoff = now - std::time::Duration::from_secs(window_secs);

        let attempts = map.entry(ip.to_string()).or_default();
        attempts.retain(|t| *t > cutoff);
        attempts.push(now);

        attempts.len() as u32 >= max_attempts
    }

    /// Check if an IP is currently locked out (without recording).
    pub fn is_locked_out(&self, ip: &str, max_attempts: u32, window_secs: u64) -> bool {
        let mut map = self.attempts.lock().unwrap();
        let now = Instant::now();
        let cutoff = now - std::time::Duration::from_secs(window_secs);

        if let Some(attempts) = map.get_mut(ip) {
            attempts.retain(|t| *t > cutoff);
            attempts.len() as u32 >= max_attempts
        } else {
            false
        }
    }

    /// Clear attempts for an IP (on successful login).
    pub fn clear(&self, ip: &str) {
        let mut map = self.attempts.lock().unwrap();
        map.remove(ip);
    }
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// POST /api/dashboard/auth/login
pub async fn login(
    State(state): State<AppState>,
    axum::Extension(ctx): axum::Extension<prism_core::context::RequestContext>,
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

    let client_ip = ctx.client_ip.clone().unwrap_or_default();

    // Enforce localhost-only access
    if dashboard.localhost_only {
        let is_local = client_ip == "127.0.0.1" || client_ip == "::1" || client_ip == "localhost";
        if !is_local {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({
                    "error": "access_denied",
                    "message": "Dashboard access restricted to localhost",
                })),
            );
        }
    }

    // Check login rate limit
    if dashboard.max_login_attempts > 0
        && state.login_limiter.is_locked_out(
            &client_ip,
            dashboard.max_login_attempts,
            dashboard.login_lockout_secs,
        )
    {
        tracing::warn!(
            client_ip = %client_ip,
            "Dashboard login rejected: IP locked out due to too many failed attempts"
        );
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({
                "error": "too_many_attempts",
                "message": format!("Too many login attempts. Try again in {} seconds.", dashboard.login_lockout_secs),
            })),
        );
    }

    // Verify username
    if body.username != dashboard.username {
        tracing::warn!(
            client_ip = %client_ip,
            username = %body.username,
            "Dashboard login failed: invalid username"
        );
        if dashboard.max_login_attempts > 0 {
            state.login_limiter.record_failure(
                &client_ip,
                dashboard.max_login_attempts,
                dashboard.login_lockout_secs,
            );
        }
        return (
            StatusCode::UNAUTHORIZED,
            Json(
                json!({"error": "invalid_credentials", "message": "Invalid username or password"}),
            ),
        );
    }

    // Verify password against bcrypt hash
    let password_valid = if dashboard.password_hash.is_empty() {
        false
    } else {
        match bcrypt::verify(&body.password, &dashboard.password_hash) {
            Ok(valid) => valid,
            Err(e) => {
                tracing::error!("bcrypt verification error: {e}");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "auth_error", "message": "Password verification failed"})),
                );
            }
        }
    };
    if !password_valid {
        tracing::warn!(
            client_ip = %client_ip,
            username = %body.username,
            "Dashboard login failed: invalid password"
        );
        if dashboard.max_login_attempts > 0 {
            state.login_limiter.record_failure(
                &client_ip,
                dashboard.max_login_attempts,
                dashboard.login_lockout_secs,
            );
        }
        return (
            StatusCode::UNAUTHORIZED,
            Json(
                json!({"error": "invalid_credentials", "message": "Invalid username or password"}),
            ),
        );
    }

    // Successful login — clear rate limit attempts
    state.login_limiter.clear(&client_ip);

    tracing::info!(
        client_ip = %client_ip,
        username = %body.username,
        "Dashboard login successful"
    );

    let secret = match dashboard.resolve_jwt_secret() {
        Some(s) => s,
        None => {
            tracing::error!("Dashboard JWT secret not configured");
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
        Err(e) => {
            tracing::error!("Failed to generate JWT token: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "token_error", "message": "Failed to generate token"})),
            )
        }
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
