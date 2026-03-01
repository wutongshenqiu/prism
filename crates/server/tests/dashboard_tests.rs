use ai_proxy_core::config::{Config, DashboardConfig, RoutingStrategy};
use ai_proxy_core::cost::CostCalculator;
use ai_proxy_core::metrics::Metrics;
use ai_proxy_core::rate_limit::RateLimiter;
use ai_proxy_core::request_log::RequestLogStore;
use ai_proxy_provider::build_registry;
use ai_proxy_provider::routing::CredentialRouter;
use ai_proxy_server::{AppState, build_router};
use arc_swap::ArcSwap;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tower::ServiceExt;

// ---------------------------------------------------------------------------
// Helper: build a test AppState backed by a real temp config file
// ---------------------------------------------------------------------------

struct TestHarness {
    state: AppState,
    _temp_dir: tempfile::TempDir,
}

fn create_test_harness() -> TestHarness {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let config_path = temp_dir.path().join("config.yaml");

    let password_hash = bcrypt::hash("test123", 4).expect("failed to hash password");

    let config = Config {
        dashboard: DashboardConfig {
            enabled: true,
            username: "admin".to_string(),
            password_hash,
            jwt_secret: Some("test-secret".to_string()),
            jwt_ttl_secs: 3600,
            request_log_capacity: 1000,
        },
        ..Config::default()
    };

    // Write the config to the temp file so update_config_file can read it back
    let yaml = serde_yml::to_string(&config).expect("failed to serialize config");
    std::fs::write(&config_path, &yaml).expect("failed to write config");

    let config_arc = Arc::new(ArcSwap::new(Arc::new(config.clone())));
    let credential_router = Arc::new(CredentialRouter::new(RoutingStrategy::RoundRobin));
    credential_router.update_from_config(&config);

    let executors = Arc::new(build_registry(None));
    let translators = Arc::new(ai_proxy_translator::build_registry());
    let metrics = Arc::new(Metrics::new());
    let request_logs = Arc::new(RequestLogStore::new(1000));

    let state = AppState {
        config: config_arc,
        router: credential_router.clone(),
        executors,
        translators,
        metrics,
        request_logs,
        config_path: Arc::new(Mutex::new(config_path.to_str().unwrap().to_string())),
        credential_router,
        rate_limiter: Arc::new(RateLimiter::new(&config.rate_limit)),
        cost_calculator: Arc::new(CostCalculator::new(&config.model_prices)),
        start_time: Instant::now(),
    };

    TestHarness {
        state,
        _temp_dir: temp_dir,
    }
}

/// Helper: send a request to the router and return (status, body as Value).
async fn send_request(harness: &TestHarness, request: Request<Body>) -> (StatusCode, Value) {
    let router = build_router(harness.state.clone());
    let response = router.oneshot(request).await.expect("request failed");
    let status = response.status();
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("failed to read body");
    let value: Value = serde_json::from_slice(&body_bytes).unwrap_or(json!({}));
    (status, value)
}

/// Helper: login and return a JWT token string.
async fn login_and_get_token(harness: &TestHarness) -> String {
    let req = Request::builder()
        .method("POST")
        .uri("/api/dashboard/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"username": "admin", "password": "test123"}).to_string(),
        ))
        .unwrap();

    let (status, body) = send_request(harness, req).await;
    assert_eq!(status, StatusCode::OK, "login failed: {body:?}");
    body["token"]
        .as_str()
        .expect("no token in login response")
        .to_string()
}

/// Helper: build a GET request with JWT auth.
fn authed_get(uri: &str, token: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

/// Helper: build a POST request with JWT auth and JSON body.
fn authed_post(uri: &str, token: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(body.to_string()))
        .unwrap()
}

/// Helper: build a PATCH request with JWT auth and JSON body.
fn authed_patch(uri: &str, token: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("PATCH")
        .uri(uri)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(body.to_string()))
        .unwrap()
}

/// Helper: build a DELETE request with JWT auth.
fn authed_delete(uri: &str, token: &str) -> Request<Body> {
    Request::builder()
        .method("DELETE")
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

// ===========================================================================
// Auth tests
// ===========================================================================

#[tokio::test]
async fn test_login_with_valid_credentials() {
    let harness = create_test_harness();
    let req = Request::builder()
        .method("POST")
        .uri("/api/dashboard/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"username": "admin", "password": "test123"}).to_string(),
        ))
        .unwrap();

    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body["token"].is_string(),
        "response should contain a JWT token"
    );
    assert_eq!(body["token_type"], "Bearer");
    assert_eq!(body["expires_in"], 3600);
}

#[tokio::test]
async fn test_login_with_invalid_password() {
    let harness = create_test_harness();
    let req = Request::builder()
        .method("POST")
        .uri("/api/dashboard/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"username": "admin", "password": "wrong-password"}).to_string(),
        ))
        .unwrap();

    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "invalid_credentials");
}

#[tokio::test]
async fn test_login_with_invalid_username() {
    let harness = create_test_harness();
    let req = Request::builder()
        .method("POST")
        .uri("/api/dashboard/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"username": "nobody", "password": "test123"}).to_string(),
        ))
        .unwrap();

    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "invalid_credentials");
}

#[tokio::test]
async fn test_protected_endpoint_without_token() {
    let harness = create_test_harness();
    let req = Request::builder()
        .method("GET")
        .uri("/api/dashboard/providers")
        .body(Body::empty())
        .unwrap();

    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "missing_token");
}

#[tokio::test]
async fn test_protected_endpoint_with_valid_token() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let req = authed_get("/api/dashboard/providers", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["providers"].is_array());
}

#[tokio::test]
async fn test_protected_endpoint_with_invalid_token() {
    let harness = create_test_harness();
    let req = Request::builder()
        .method("GET")
        .uri("/api/dashboard/providers")
        .header("authorization", "Bearer invalid.jwt.token")
        .body(Body::empty())
        .unwrap();

    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "invalid_token");
}

#[tokio::test]
async fn test_protected_endpoint_with_expired_token() {
    let harness = create_test_harness();

    // Generate a token that already expired (ttl = 0 means exp == iat, immediately expired)
    let expired_token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &serde_json::json!({
            "sub": "admin",
            "iat": 1_000_000,
            "exp": 1_000_001, // far in the past
        }),
        &jsonwebtoken::EncodingKey::from_secret(b"test-secret"),
    )
    .unwrap();

    let req = Request::builder()
        .method("GET")
        .uri("/api/dashboard/providers")
        .header("authorization", format!("Bearer {expired_token}"))
        .body(Body::empty())
        .unwrap();

    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "token_expired");
}

#[tokio::test]
async fn test_login_with_dashboard_disabled() {
    let harness = create_test_harness();

    // Disable dashboard in config
    let mut config = (*harness.state.config.load_full()).clone();
    config.dashboard.enabled = false;
    harness.state.config.store(Arc::new(config));

    let req = Request::builder()
        .method("POST")
        .uri("/api/dashboard/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"username": "admin", "password": "test123"}).to_string(),
        ))
        .unwrap();

    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "not_found");
}

#[tokio::test]
async fn test_token_refresh() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let req = authed_post("/api/dashboard/auth/refresh", &token, json!({}));
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["token"].is_string());
    assert_eq!(body["token_type"], "Bearer");
    // The new token should be different from the original
    let new_token = body["token"].as_str().unwrap();
    // (Both are valid JWT tokens signed with the same secret, but issued at different times)
    assert!(new_token.starts_with("ey"));
}

// ===========================================================================
// Provider CRUD tests
// ===========================================================================

#[tokio::test]
async fn test_list_providers_empty() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let req = authed_get("/api/dashboard/providers", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    let providers = body["providers"]
        .as_array()
        .expect("providers should be array");
    assert!(
        providers.is_empty(),
        "initially there should be no providers"
    );
}

#[tokio::test]
async fn test_create_provider_and_list() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    // Create a provider
    let create_body = json!({
        "provider_type": "openai",
        "api_key": "sk-test-key-1234567890abcdef",
        "name": "Test OpenAI"
    });
    let req = authed_post("/api/dashboard/providers", &token, create_body);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::CREATED, "create failed: {body:?}");

    // Reload config into state so list sees the new provider
    let config_path = harness.state.config_path.lock().unwrap().clone();
    let new_config = Config::load(&config_path).expect("failed to reload config");
    harness.state.config.store(Arc::new(new_config));

    // List providers
    let req = authed_get("/api/dashboard/providers", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    let providers = body["providers"].as_array().unwrap();
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0]["provider_type"], "openai");
    assert_eq!(providers[0]["name"], "Test OpenAI");
    assert_eq!(providers[0]["id"], "openai-0");
    // API key should be masked
    let masked = providers[0]["api_key_masked"].as_str().unwrap();
    assert!(masked.contains("****"), "API key should be masked");
    assert!(
        !masked.contains("sk-test-key-1234567890abcdef"),
        "full key should not appear"
    );
}

#[tokio::test]
async fn test_get_provider_by_id() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    // Create a provider
    let create_body = json!({
        "provider_type": "claude",
        "api_key": "sk-ant-test-1234567890abcdef",
        "name": "Test Claude Provider",
        "base_url": "https://api.anthropic.com"
    });
    let req = authed_post("/api/dashboard/providers", &token, create_body);
    let (status, _) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::CREATED);

    // Reload config
    let config_path = harness.state.config_path.lock().unwrap().clone();
    let new_config = Config::load(&config_path).expect("failed to reload config");
    harness.state.config.store(Arc::new(new_config));

    // Get the provider by ID
    let req = authed_get("/api/dashboard/providers/claude-0", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["provider_type"], "claude");
    assert_eq!(body["name"], "Test Claude Provider");
    assert_eq!(body["base_url"], "https://api.anthropic.com");
    // API key should be masked
    let masked = body["api_key_masked"].as_str().unwrap();
    assert!(masked.contains("****"));
}

#[tokio::test]
async fn test_get_provider_not_found() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let req = authed_get("/api/dashboard/providers/openai-99", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "not_found");
}

#[tokio::test]
async fn test_update_provider() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    // Create a provider
    let create_body = json!({
        "provider_type": "openai",
        "api_key": "sk-test-key-1234567890abcdef",
        "name": "Original Name"
    });
    let req = authed_post("/api/dashboard/providers", &token, create_body);
    let (status, _) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::CREATED);

    // Reload config
    let config_path = harness.state.config_path.lock().unwrap().clone();
    let new_config = Config::load(&config_path).expect("failed to reload config");
    harness.state.config.store(Arc::new(new_config));

    // Update the provider
    let update_body = json!({
        "name": "Updated Name",
        "disabled": true
    });
    let req = authed_patch("/api/dashboard/providers/openai-0", &token, update_body);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK, "update failed: {body:?}");

    // Reload and verify
    let config_path = harness.state.config_path.lock().unwrap().clone();
    let new_config = Config::load(&config_path).expect("failed to reload config");
    harness.state.config.store(Arc::new(new_config));

    let req = authed_get("/api/dashboard/providers/openai-0", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["name"], "Updated Name");
    assert_eq!(body["disabled"], true);
}

#[tokio::test]
async fn test_delete_provider() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    // Create a provider
    let create_body = json!({
        "provider_type": "gemini",
        "api_key": "gemini-test-key-1234567890abcdef",
    });
    let req = authed_post("/api/dashboard/providers", &token, create_body);
    let (status, _) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::CREATED);

    // Reload config
    let config_path = harness.state.config_path.lock().unwrap().clone();
    let new_config = Config::load(&config_path).expect("failed to reload config");
    harness.state.config.store(Arc::new(new_config));

    // Verify it exists
    let req = authed_get("/api/dashboard/providers", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["providers"].as_array().unwrap().len(), 1);

    // Delete the provider
    let req = authed_delete("/api/dashboard/providers/gemini-0", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK, "delete failed: {body:?}");

    // Reload config and verify deletion
    let config_path = harness.state.config_path.lock().unwrap().clone();
    let new_config = Config::load(&config_path).expect("failed to reload config");
    harness.state.config.store(Arc::new(new_config));

    let req = authed_get("/api/dashboard/providers", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["providers"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_create_provider_with_empty_api_key() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let create_body = json!({
        "provider_type": "openai",
        "api_key": "",
    });
    let req = authed_post("/api/dashboard/providers", &token, create_body);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], "validation_failed");
}

#[tokio::test]
async fn test_create_provider_with_invalid_type() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let create_body = json!({
        "provider_type": "invalid-provider",
        "api_key": "some-key-that-is-long-enough",
    });
    let req = authed_post("/api/dashboard/providers", &token, create_body);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], "validation_failed");
}

#[tokio::test]
async fn test_create_openai_compat_provider() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let create_body = json!({
        "provider_type": "openai-compat",
        "api_key": "deepseek-test-key-1234567890abcdef",
        "base_url": "https://api.deepseek.com/v1",
        "name": "DeepSeek"
    });
    let req = authed_post("/api/dashboard/providers", &token, create_body);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "create openai-compat failed: {body:?}"
    );

    // Reload and verify
    let config_path = harness.state.config_path.lock().unwrap().clone();
    let new_config = Config::load(&config_path).expect("failed to reload config");
    harness.state.config.store(Arc::new(new_config));

    let req = authed_get("/api/dashboard/providers/openai-compat-0", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["provider_type"], "openai-compat");
    assert_eq!(body["name"], "DeepSeek");
    assert_eq!(body["base_url"], "https://api.deepseek.com/v1");
}

// ===========================================================================
// Auth key tests
// ===========================================================================

#[tokio::test]
async fn test_list_auth_keys_empty() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let req = authed_get("/api/dashboard/auth-keys", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    let keys = body["auth_keys"]
        .as_array()
        .expect("auth_keys should be array");
    assert!(keys.is_empty());
}

#[tokio::test]
async fn test_create_auth_key() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let req = authed_post("/api/dashboard/auth-keys", &token, json!({}));
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "create auth key failed: {body:?}"
    );
    let key = body["key"]
        .as_str()
        .expect("response should contain full key");
    assert!(
        key.starts_with("sk-proxy-"),
        "key should start with sk-proxy-"
    );
    assert!(key.len() > 10, "key should be reasonably long");
}

#[tokio::test]
async fn test_create_and_list_auth_keys() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    // Create a key
    let req = authed_post("/api/dashboard/auth-keys", &token, json!({}));
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::CREATED);
    let full_key = body["key"].as_str().unwrap().to_string();

    // Reload config into state
    let config_path = harness.state.config_path.lock().unwrap().clone();
    let new_config = Config::load(&config_path).expect("failed to reload config");
    harness.state.config.store(Arc::new(new_config));

    // List keys
    let req = authed_get("/api/dashboard/auth-keys", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    let keys = body["auth_keys"].as_array().unwrap();
    assert_eq!(keys.len(), 1);
    // Key should be masked in listing
    let masked = keys[0]["key_masked"].as_str().unwrap();
    assert!(masked.contains("****"));
    assert_ne!(
        masked, &full_key,
        "listed key should be masked, not the full key"
    );
}

#[tokio::test]
async fn test_delete_auth_key() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    // Create a key
    let req = authed_post("/api/dashboard/auth-keys", &token, json!({}));
    let (status, _) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::CREATED);

    // Reload config
    let config_path = harness.state.config_path.lock().unwrap().clone();
    let new_config = Config::load(&config_path).expect("failed to reload config");
    harness.state.config.store(Arc::new(new_config));

    // Delete the key (id = 0)
    let req = authed_delete("/api/dashboard/auth-keys/0", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK, "delete auth key failed: {body:?}");

    // Reload and verify deletion
    let config_path = harness.state.config_path.lock().unwrap().clone();
    let new_config = Config::load(&config_path).expect("failed to reload config");
    harness.state.config.store(Arc::new(new_config));

    let req = authed_get("/api/dashboard/auth-keys", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["auth_keys"].as_array().unwrap().is_empty());
}

// ===========================================================================
// Routing tests
// ===========================================================================

#[tokio::test]
async fn test_get_routing() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let req = authed_get("/api/dashboard/routing", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    // Default strategy is round-robin
    assert_eq!(body["strategy"], "round-robin");
}

#[tokio::test]
async fn test_update_routing_strategy() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    // Update to fill-first
    let req = authed_patch(
        "/api/dashboard/routing",
        &token,
        json!({"strategy": "fill-first"}),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK, "update routing failed: {body:?}");

    // Reload config and verify
    let config_path = harness.state.config_path.lock().unwrap().clone();
    let new_config = Config::load(&config_path).expect("failed to reload config");
    harness.state.config.store(Arc::new(new_config));

    let req = authed_get("/api/dashboard/routing", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["strategy"], "fill-first");
}

#[tokio::test]
async fn test_update_routing_round_robin() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    // Set to fill-first first
    let req = authed_patch(
        "/api/dashboard/routing",
        &token,
        json!({"strategy": "fill-first"}),
    );
    let (status, _) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);

    // Reload
    let config_path = harness.state.config_path.lock().unwrap().clone();
    let new_config = Config::load(&config_path).expect("failed to reload config");
    harness.state.config.store(Arc::new(new_config));

    // Set back to round-robin
    let req = authed_patch(
        "/api/dashboard/routing",
        &token,
        json!({"strategy": "round-robin"}),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK, "update routing failed: {body:?}");

    // Reload and verify
    let config_path = harness.state.config_path.lock().unwrap().clone();
    let new_config = Config::load(&config_path).expect("failed to reload config");
    harness.state.config.store(Arc::new(new_config));

    let req = authed_get("/api/dashboard/routing", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["strategy"], "round-robin");
}

#[tokio::test]
async fn test_update_routing_invalid_strategy() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let req = authed_patch(
        "/api/dashboard/routing",
        &token,
        json!({"strategy": "random"}),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], "validation_failed");
}

// ===========================================================================
// Request logs tests
// ===========================================================================

#[tokio::test]
async fn test_query_logs_empty() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let req = authed_get("/api/dashboard/logs", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 0);
    assert!(body["items"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_log_stats_empty() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let req = authed_get("/api/dashboard/logs/stats", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total_entries"], 0);
    assert_eq!(body["error_count"], 0);
    assert_eq!(body["avg_latency_ms"], 0);
}

#[tokio::test]
async fn test_log_stats_with_entries() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    // Push some log entries directly
    harness
        .state
        .request_logs
        .push(ai_proxy_core::request_log::RequestLogEntry {
            timestamp: chrono::Utc::now().timestamp_millis(),
            request_id: "req-1".to_string(),
            method: "POST".to_string(),
            path: "/v1/chat/completions".to_string(),
            status: 200,
            latency_ms: 150,
            provider: Some("openai".to_string()),
            model: Some("gpt-4".to_string()),
            input_tokens: Some(100),
            output_tokens: Some(50),
            error: None,
            cost: None,
        });
    harness
        .state
        .request_logs
        .push(ai_proxy_core::request_log::RequestLogEntry {
            timestamp: chrono::Utc::now().timestamp_millis(),
            request_id: "req-2".to_string(),
            method: "POST".to_string(),
            path: "/v1/chat/completions".to_string(),
            status: 500,
            latency_ms: 50,
            provider: Some("claude".to_string()),
            model: Some("claude-3".to_string()),
            input_tokens: None,
            output_tokens: None,
            error: Some("Internal Server Error".to_string()),
            cost: None,
        });

    let req = authed_get("/api/dashboard/logs/stats", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total_entries"], 2);
    assert_eq!(body["error_count"], 1);
    assert_eq!(body["capacity"], 1000);
}

#[tokio::test]
async fn test_query_logs_with_entries() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    // Push log entries
    for i in 0..5 {
        harness
            .state
            .request_logs
            .push(ai_proxy_core::request_log::RequestLogEntry {
                timestamp: chrono::Utc::now().timestamp_millis(),
                request_id: format!("req-{i}"),
                method: "POST".to_string(),
                path: "/v1/chat/completions".to_string(),
                status: if i % 2 == 0 { 200 } else { 429 },
                latency_ms: 100,
                provider: Some("openai".to_string()),
                model: Some("gpt-4".to_string()),
                input_tokens: Some(10),
                output_tokens: Some(20),
                error: if i % 2 != 0 {
                    Some("rate limited".to_string())
                } else {
                    None
                },
                cost: None,
            });
    }

    let req = authed_get("/api/dashboard/logs", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 5);
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 5);
}

// ===========================================================================
// System tests
// ===========================================================================

#[tokio::test]
async fn test_system_health() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let req = authed_get("/api/dashboard/system/health", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert!(body["uptime_secs"].is_number());
    assert!(body["host"].is_string());
    assert!(body["port"].is_number());
    assert!(body["providers"].is_object());
}

#[tokio::test]
async fn test_system_logs() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    // System logs endpoint reads from log directory -- should handle missing dir gracefully
    let req = authed_get("/api/dashboard/system/logs", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    // Even with no log directory, should return a valid response
    assert!(body["logs"].is_array());
    assert_eq!(body["total"], 0);
}

// ===========================================================================
// Config ops tests
// ===========================================================================

#[tokio::test]
async fn test_get_current_config() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let req = authed_get("/api/dashboard/config/current", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    // Check sanitized config fields
    assert!(body["host"].is_string());
    assert!(body["port"].is_number());
    assert!(body["dashboard"].is_object());
    assert_eq!(body["dashboard"]["enabled"], true);
    assert_eq!(body["dashboard"]["username"], "admin");
    // Password hash and jwt_secret should not be in the sanitized output
    assert!(body["dashboard"]["password_hash"].is_null());
    assert!(body["dashboard"]["jwt_secret"].is_null());
    // Provider counts
    assert!(body["providers"].is_object());
}

#[tokio::test]
async fn test_reload_config() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let req = authed_post("/api/dashboard/config/reload", &token, json!({}));
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK, "reload failed: {body:?}");
    assert_eq!(body["message"], "Configuration reloaded successfully");
}

#[tokio::test]
async fn test_validate_config_valid() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    // A minimal valid config as JSON
    let valid_config = json!({
        "host": "0.0.0.0",
        "port": 8080
    });
    let req = authed_post("/api/dashboard/config/validate", &token, valid_config);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK, "validate failed: {body:?}");
    assert_eq!(body["valid"], true);
}

#[tokio::test]
async fn test_validate_config_invalid() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    // Invalid config: port as string instead of number
    let invalid_config = json!({
        "host": "0.0.0.0",
        "port": "not-a-number"
    });
    let req = authed_post("/api/dashboard/config/validate", &token, invalid_config);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["valid"], false);
    assert_eq!(body["error"], "validation_failed");
}

// ===========================================================================
// Token via query parameter tests
// ===========================================================================

#[tokio::test]
async fn test_protected_endpoint_with_token_query_param() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    // Access protected endpoint with token as query parameter
    let uri = format!("/api/dashboard/providers?token={token}");
    let req = Request::builder()
        .method("GET")
        .uri(&uri)
        .body(Body::empty())
        .unwrap();

    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["providers"].is_array());
}

// ===========================================================================
// Multiple provider types coexistence test
// ===========================================================================

#[tokio::test]
async fn test_multiple_provider_types() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    // Create providers of different types
    let providers = vec![
        json!({
            "provider_type": "openai",
            "api_key": "sk-openai-test-1234567890abcdef",
            "name": "OpenAI Prod"
        }),
        json!({
            "provider_type": "claude",
            "api_key": "sk-ant-claude-test-1234567890abcdef",
            "name": "Claude Prod"
        }),
        json!({
            "provider_type": "gemini",
            "api_key": "gemini-key-test-1234567890abcdef",
            "name": "Gemini Prod"
        }),
    ];

    for p in &providers {
        let req = authed_post("/api/dashboard/providers", &token, p.clone());
        let (status, body) = send_request(&harness, req).await;
        assert_eq!(status, StatusCode::CREATED, "create failed: {body:?}");

        // Reload config after each creation so the next write reads current state
        let config_path = harness.state.config_path.lock().unwrap().clone();
        let new_config = Config::load(&config_path).expect("failed to reload config");
        harness.state.config.store(Arc::new(new_config));
    }

    // List all providers
    let req = authed_get("/api/dashboard/providers", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    let all_providers = body["providers"].as_array().unwrap();
    assert_eq!(all_providers.len(), 3);

    // Verify provider IDs
    let ids: Vec<&str> = all_providers
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"openai-0"));
    assert!(ids.contains(&"claude-0"));
    assert!(ids.contains(&"gemini-0"));
}
