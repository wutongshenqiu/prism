use arc_swap::ArcSwap;
use axum::Json;
use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::routing::{get, post};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{Duration as ChronoDuration, Utc};
use prism_core::auth_key::AuthKeyEntry;
use prism_core::auth_profile::{AuthMode, AuthProfileEntry};
use prism_core::config::{Config, DashboardConfig};
use prism_core::cost::CostCalculator;
use prism_core::memory_log_store::InMemoryLogStore;
use prism_core::metrics::Metrics;
use prism_core::provider::{Format, UpstreamKind, WireApi};
use prism_core::rate_limit::CompositeRateLimiter;
use prism_core::request_log::LogStore;
use prism_core::request_record::{AttemptSummary, RequestRecord, TokenUsage};
use prism_core::routing::config::{RouteMatch, RouteRule, RoutingConfig};
use prism_provider::build_registry;
use prism_provider::catalog::ProviderCatalog;
use prism_provider::health::HealthManager;
use prism_provider::routing::CredentialRouter;
use prism_server::{AppState, build_router};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
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
    create_test_harness_with_auth_runtime(Arc::new(
        prism_server::auth_runtime::AuthRuntimeManager::new(),
    ))
}

fn create_test_harness_with_auth_runtime(
    auth_runtime: Arc<prism_server::auth_runtime::AuthRuntimeManager>,
) -> TestHarness {
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
            localhost_only: false,
            ..DashboardConfig::default()
        },
        ..Config::default()
    };

    // Write the config to the temp file so update_config_file can read it back
    let yaml = config.to_yaml().expect("failed to serialize config");
    std::fs::write(&config_path, &yaml).expect("failed to write config");

    auth_runtime
        .initialize(config_path.to_str().unwrap(), &config)
        .expect("failed to initialize auth runtime");

    let config_arc = Arc::new(ArcSwap::new(Arc::new(config.clone())));
    let credential_router = Arc::new(CredentialRouter::new(Default::default()));
    credential_router.set_oauth_states(auth_runtime.oauth_snapshot());
    credential_router.update_from_config(&config);

    let http_client_pool = Arc::new(prism_core::proxy::HttpClientPool::new());
    let executors = Arc::new(build_registry(None, http_client_pool.clone()));
    let translators = Arc::new(prism_translator::build_registry());
    let metrics = Arc::new(Metrics::new());
    let log_store: Arc<dyn LogStore> = Arc::new(InMemoryLogStore::new(1000, None));
    let catalog = Arc::new(ProviderCatalog::new());
    catalog.update_from_credentials(&credential_router.credential_map());

    let state = AppState {
        config: config_arc,
        router: credential_router.clone(),
        executors,
        translators,
        metrics,
        log_store,
        config_path: Arc::new(Mutex::new(config_path.to_str().unwrap().to_string())),
        rate_limiter: Arc::new(CompositeRateLimiter::new(&config.rate_limit)),
        cost_calculator: Arc::new(CostCalculator::new(&config.model_prices)),
        response_cache: None,
        thinking_cache: None,
        http_client_pool,
        start_time: Instant::now(),
        login_limiter: Arc::new(prism_server::handler::dashboard::auth::LoginRateLimiter::new()),
        catalog,
        health_manager: Arc::new(HealthManager::new(Default::default())),
        auth_runtime,
        oauth_sessions: Arc::new(dashmap::DashMap::new()),
        device_sessions: Arc::new(dashmap::DashMap::new()),
        provider_probe_cache: Arc::new(dashmap::DashMap::new()),
    };

    TestHarness {
        state,
        _temp_dir: temp_dir,
    }
}

fn reload_runtime_config(harness: &TestHarness) {
    let config_path = harness.state.config_path.lock().unwrap().clone();
    let new_config = Config::load(&config_path).expect("failed to reload config");
    harness
        .state
        .auth_runtime
        .sync_with_config(&new_config)
        .expect("failed to sync auth runtime");
    harness
        .state
        .router
        .set_oauth_states(harness.state.auth_runtime.oauth_snapshot());
    harness.state.router.update_from_config(&new_config);
    harness
        .state
        .catalog
        .update_from_credentials(&harness.state.router.credential_map());
    harness.state.config.store(Arc::new(new_config));
}

fn read_auth_runtime_store(harness: &TestHarness) -> Value {
    let Some(store_dir) = harness
        .state
        .auth_runtime
        .storage_dir()
        .expect("failed to read auth runtime dir")
    else {
        return json!({
            "version": 1,
            "oauth_profiles": []
        });
    };
    if !store_dir.exists() {
        return json!({
            "version": 1,
            "oauth_profiles": []
        });
    }

    let mut oauth_profiles = std::fs::read_dir(store_dir)
        .expect("failed to read auth runtime dir")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            (path.extension().and_then(|ext| ext.to_str()) == Some("json")).then_some(path)
        })
        .map(|path| {
            let contents =
                std::fs::read_to_string(&path).expect("failed to read auth runtime store file");
            serde_json::from_str::<Value>(&contents).expect("failed to parse auth runtime store")
        })
        .collect::<Vec<_>>();
    oauth_profiles.sort_by(|a, b| {
        let left = format!(
            "{}/{}",
            a["provider"].as_str().unwrap_or_default(),
            a["profile_id"].as_str().unwrap_or_default()
        );
        let right = format!(
            "{}/{}",
            b["provider"].as_str().unwrap_or_default(),
            b["profile_id"].as_str().unwrap_or_default()
        );
        left.cmp(&right)
    });

    json!({
        "version": 1,
        "oauth_profiles": oauth_profiles,
    })
}

#[derive(Clone)]
struct MockOauthState {
    token_response: Value,
}

struct MockCodexOauthServer {
    auth_url: String,
    token_url: String,
    _task: tokio::task::JoinHandle<()>,
}

struct RotatingCodexOauthServer {
    auth_url: String,
    token_url: String,
    refresh_requests: Arc<AtomicUsize>,
    _task: tokio::task::JoinHandle<()>,
}

struct MockCodexDeviceServer {
    user_code_url: String,
    token_url: String,
    verification_url: String,
    _task: tokio::task::JoinHandle<()>,
}

struct MockOpenAiProbeServer {
    base_url: String,
    model_requests: Arc<AtomicUsize>,
    chat_requests: Arc<AtomicUsize>,
    responses_requests: Arc<AtomicUsize>,
    _task: tokio::task::JoinHandle<()>,
}

async fn spawn_mock_codex_oauth_server(token_response: Value) -> MockCodexOauthServer {
    async fn authorize() -> Json<Value> {
        Json(json!({"ok": true}))
    }

    async fn token(State(state): State<MockOauthState>, body: String) -> (StatusCode, Json<Value>) {
        assert!(
            body.contains("client_id="),
            "expected oauth client_id in token request: {body}"
        );
        (StatusCode::OK, Json(state.token_response))
    }

    let state = MockOauthState { token_response };
    let app = Router::new()
        .route("/oauth/authorize", get(authorize))
        .route("/oauth/token", post(token))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock oauth listener");
    let addr = listener.local_addr().expect("mock oauth addr");
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("mock oauth server");
    });

    MockCodexOauthServer {
        auth_url: format!("http://{addr}/oauth/authorize"),
        token_url: format!("http://{addr}/oauth/token"),
        _task: task,
    }
}

async fn spawn_rotating_mock_codex_oauth_server() -> RotatingCodexOauthServer {
    #[derive(Clone)]
    struct RotatingOauthState {
        refresh_requests: Arc<AtomicUsize>,
    }

    async fn authorize() -> Json<Value> {
        Json(json!({"ok": true}))
    }

    async fn token(
        State(state): State<RotatingOauthState>,
        body: String,
    ) -> (StatusCode, Json<Value>) {
        assert!(
            body.contains("client_id="),
            "expected oauth client_id in token request: {body}"
        );
        if body.contains("grant_type=refresh_token") {
            let attempt = state.refresh_requests.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            if attempt == 0 {
                return (
                    StatusCode::OK,
                    Json(json!({
                        "access_token": "refreshed-access-token",
                        "refresh_token": "next-refresh-token",
                        "expires_in": 7200
                    })),
                );
            }
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": {
                        "message": "Your refresh token has already been used to generate a new access token. Please try signing in again.",
                        "type": "invalid_request_error",
                        "param": null,
                        "code": "refresh_token_reused"
                    }
                })),
            );
        }
        (
            StatusCode::OK,
            Json(json!({
                "access_token": "oauth-access-token",
                "refresh_token": "oauth-refresh-token",
                "expires_in": 7200
            })),
        )
    }

    let refresh_requests = Arc::new(AtomicUsize::new(0));
    let app = Router::new()
        .route("/oauth/authorize", get(authorize))
        .route("/oauth/token", post(token))
        .with_state(RotatingOauthState {
            refresh_requests: refresh_requests.clone(),
        });

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind rotating mock oauth listener");
    let addr = listener.local_addr().expect("rotating mock oauth addr");
    let task = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("rotating mock oauth server");
    });

    RotatingCodexOauthServer {
        auth_url: format!("http://{addr}/oauth/authorize"),
        token_url: format!("http://{addr}/oauth/token"),
        refresh_requests,
        _task: task,
    }
}

async fn spawn_mock_codex_device_server() -> MockCodexDeviceServer {
    #[derive(Clone)]
    struct DeviceState {
        polls: Arc<AtomicUsize>,
    }

    async fn user_code() -> Json<Value> {
        Json(json!({
            "device_auth_id": "device-auth-123",
            "user_code": "CODE-ABCD",
            "interval": 1
        }))
    }

    async fn token(State(state): State<DeviceState>) -> (StatusCode, Json<Value>) {
        let poll = state.polls.fetch_add(1, Ordering::SeqCst);
        if poll == 0 {
            return (StatusCode::NOT_FOUND, Json(json!({"status": "pending"})));
        }
        (
            StatusCode::OK,
            Json(json!({
                "authorization_code": "device-authorization-code",
                "code_verifier": "device-code-verifier"
            })),
        )
    }

    let app = Router::new()
        .route("/device/usercode", post(user_code))
        .route("/device/token", post(token))
        .with_state(DeviceState {
            polls: Arc::new(AtomicUsize::new(0)),
        });

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock device listener");
    let addr = listener.local_addr().expect("mock device addr");
    let task = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("mock device server");
    });

    MockCodexDeviceServer {
        user_code_url: format!("http://{addr}/device/usercode"),
        token_url: format!("http://{addr}/device/token"),
        verification_url: format!("http://{addr}/device/verify"),
        _task: task,
    }
}

async fn spawn_mock_openai_probe_server() -> MockOpenAiProbeServer {
    #[derive(Clone)]
    struct ProbeState {
        model_requests: Arc<AtomicUsize>,
        chat_requests: Arc<AtomicUsize>,
        responses_requests: Arc<AtomicUsize>,
    }

    async fn list_models(State(state): State<ProbeState>) -> (StatusCode, Json<Value>) {
        state.model_requests.fetch_add(1, Ordering::SeqCst);
        (StatusCode::NOT_FOUND, Json(json!({})))
    }

    async fn chat_completions(
        State(state): State<ProbeState>,
        body: String,
    ) -> (StatusCode, Json<Value>) {
        state.chat_requests.fetch_add(1, Ordering::SeqCst);
        assert!(
            body.contains("Reply with exactly ok."),
            "expected health probe payload, got {body}"
        );
        (
            StatusCode::OK,
            Json(json!({
                "id": "chatcmpl-probe",
                "object": "chat.completion",
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "ok"
                    },
                    "finish_reason": "stop"
                }]
            })),
        )
    }

    async fn responses(State(state): State<ProbeState>, body: String) -> (StatusCode, Json<Value>) {
        state.responses_requests.fetch_add(1, Ordering::SeqCst);
        assert!(
            body.contains("\"input\""),
            "expected responses payload, got {body}"
        );
        (
            StatusCode::OK,
            Json(json!({
                "id": "resp-probe",
                "object": "response",
                "output": [{
                    "type": "message",
                    "role": "assistant",
                    "content": [{
                        "type": "output_text",
                        "text": "ok"
                    }]
                }]
            })),
        )
    }

    let state = ProbeState {
        model_requests: Arc::new(AtomicUsize::new(0)),
        chat_requests: Arc::new(AtomicUsize::new(0)),
        responses_requests: Arc::new(AtomicUsize::new(0)),
    };
    let app = Router::new()
        .route("/v1/models", get(list_models))
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/responses", post(responses))
        .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock openai probe listener");
    let addr = listener.local_addr().expect("mock openai probe addr");
    let task = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("mock openai probe server");
    });

    MockOpenAiProbeServer {
        base_url: format!("http://{addr}"),
        model_requests: state.model_requests,
        chat_requests: state.chat_requests,
        responses_requests: state.responses_requests,
        _task: task,
    }
}

fn fake_id_token(email: &str, sub: &str) -> String {
    let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"none"}"#);
    let payload = URL_SAFE_NO_PAD.encode(
        json!({
            "email": email,
            "sub": sub,
            "account_id": sub,
        })
        .to_string(),
    );
    format!("{header}.{payload}.sig")
}

fn write_codex_auth_file(
    dir: &std::path::Path,
    email: &str,
    account_id: &str,
) -> std::path::PathBuf {
    let path = dir.join("codex-auth.json");
    let access_payload = URL_SAFE_NO_PAD.encode(
        json!({
            "exp": chrono::Utc::now().timestamp() + 3600,
            "sub": account_id
        })
        .to_string(),
    );
    let access_token = format!("hdr.{access_payload}.sig");
    let contents = json!({
        "tokens": {
            "access_token": access_token,
            "refresh_token": "local-refresh-token",
            "id_token": fake_id_token(email, account_id),
            "account_id": account_id
        },
        "last_refresh": chrono::Utc::now().to_rfc3339(),
    });
    std::fs::write(&path, contents.to_string()).expect("write codex auth file");
    path
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

/// Helper: create a valid JWT token string for dashboard tests.
async fn login_and_get_token(harness: &TestHarness) -> String {
    let config = harness.state.config.load();
    let secret = config
        .dashboard
        .resolve_jwt_secret()
        .expect("dashboard jwt secret");
    prism_server::middleware::dashboard_auth::generate_token(
        "admin",
        &secret,
        config.dashboard.jwt_ttl_secs,
    )
    .expect("generate dashboard jwt")
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

/// Helper: build a PUT request with JWT auth and JSON body.
fn authed_put(uri: &str, token: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("PUT")
        .uri(uri)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(body.to_string()))
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
    assert_eq!(body["authenticated"], true);
    assert_eq!(body["username"], "admin");
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

    let (status, _body) = send_request(&harness, req).await;
    // Dashboard routes are not registered when disabled, so we get a plain 404
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_token_refresh() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let req = authed_post("/api/dashboard/auth/refresh", &token, json!({}));
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["authenticated"], true);
    assert_eq!(body["username"], "admin");
    assert_eq!(body["expires_in"], 3600);
}

#[tokio::test]
async fn test_session_probe_without_dashboard_session_returns_unauthenticated() {
    let harness = create_test_harness();

    let req = Request::builder()
        .method("GET")
        .uri("/api/dashboard/auth/session")
        .body(Body::empty())
        .unwrap();

    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["authenticated"], false);
    assert!(body["username"].is_null());
}

#[tokio::test]
async fn test_session_probe_with_valid_token_returns_authenticated() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let req = authed_get("/api/dashboard/auth/session", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["authenticated"], true);
    assert_eq!(body["username"], "admin");
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
        "format": "openai",
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
    assert_eq!(providers[0]["format"], "openai");
    assert_eq!(providers[0]["name"], "Test OpenAI");
    // API key should be masked
    let masked = providers[0]["api_key_masked"].as_str().unwrap();
    assert!(masked.contains("****"), "API key should be masked");
    assert!(
        !masked.contains("sk-test-key-1234567890abcdef"),
        "full key should not appear"
    );
}

#[tokio::test]
async fn test_get_provider_by_name() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    // Create a provider
    let create_body = json!({
        "format": "claude",
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

    // Get the provider by name
    let req = authed_get("/api/dashboard/providers/Test%20Claude%20Provider", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["format"], "claude");
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

    let req = authed_get("/api/dashboard/providers/nonexistent-provider", &token);
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
        "format": "openai",
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

    // Update the provider (name is immutable, not included in update body)
    let update_body = json!({
        "disabled": true
    });
    let req = authed_patch(
        "/api/dashboard/providers/Original%20Name",
        &token,
        update_body,
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK, "update failed: {body:?}");

    // Reload and verify
    let config_path = harness.state.config_path.lock().unwrap().clone();
    let new_config = Config::load(&config_path).expect("failed to reload config");
    harness.state.config.store(Arc::new(new_config));

    let req = authed_get("/api/dashboard/providers/Original%20Name", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["name"], "Original Name");
    assert_eq!(body["disabled"], true);
}

#[tokio::test]
async fn test_update_provider_preserves_existing_codex_upstream() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let create_body = json!({
        "format": "openai",
        "upstream": "codex",
        "name": "Codex Gateway",
        "base_url": "https://chatgpt.com/backend-api/codex",
        "wire_api": "responses",
        "auth_profiles": [
            { "id": "codex-user", "mode": "codex-oauth" }
        ]
    });
    let req = authed_post("/api/dashboard/providers", &token, create_body);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::CREATED, "create failed: {body:?}");

    let config_path = harness.state.config_path.lock().unwrap().clone();
    let new_config = Config::load(&config_path).expect("failed to reload config");
    harness.state.config.store(Arc::new(new_config));

    let update_body = json!({
        "disabled": true
    });
    let req = authed_patch(
        "/api/dashboard/providers/Codex%20Gateway",
        &token,
        update_body,
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK, "update failed: {body:?}");

    let config_path = harness.state.config_path.lock().unwrap().clone();
    let new_config = Config::load(&config_path).expect("failed to reload config");
    harness.state.config.store(Arc::new(new_config));

    let req = authed_get("/api/dashboard/providers/Codex%20Gateway", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["upstream"], "codex");
    assert_eq!(body["wire_api"], "responses");
    assert_eq!(body["disabled"], true);
}

#[tokio::test]
async fn test_delete_provider() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    // Create a provider
    let create_body = json!({
        "format": "gemini",
        "api_key": "gemini-test-key-1234567890abcdef",
        "name": "Gemini Test"
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
    let req = authed_delete("/api/dashboard/providers/Gemini%20Test", &token);
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
        "format": "openai",
        "api_key": "",
        "name": "Empty Key Provider"
    });
    let req = authed_post("/api/dashboard/providers", &token, create_body);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::CREATED, "create failed: {body:?}");

    reload_runtime_config(&harness);

    let req = authed_get("/api/dashboard/providers/Empty%20Key%20Provider", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["api_key_masked"], "");
    assert_eq!(body["auth_profiles"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_create_provider_with_invalid_type() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let create_body = json!({
        "format": "invalid-provider",
        "api_key": "some-key-that-is-long-enough",
        "name": "Invalid Provider"
    });
    let req = authed_post("/api/dashboard/providers", &token, create_body);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], "validation_failed");
}

#[tokio::test]
async fn test_create_openai_provider_for_deepseek() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let create_body = json!({
        "format": "openai",
        "api_key": "deepseek-test-key-1234567890abcdef",
        "base_url": "https://api.deepseek.com/v1",
        "name": "DeepSeek"
    });
    let req = authed_post("/api/dashboard/providers", &token, create_body);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "create openai provider failed: {body:?}"
    );

    // Reload and verify
    let config_path = harness.state.config_path.lock().unwrap().clone();
    let new_config = Config::load(&config_path).expect("failed to reload config");
    harness.state.config.store(Arc::new(new_config));

    let req = authed_get("/api/dashboard/providers/DeepSeek", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["format"], "openai");
    assert_eq!(body["name"], "DeepSeek");
    assert_eq!(body["base_url"], "https://api.deepseek.com/v1");
}

#[tokio::test]
async fn test_create_provider_with_auth_profiles() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let create_body = json!({
        "format": "openai",
        "upstream": "codex",
        "name": "Codex Gateway",
        "auth_profiles": [
            {
                "id": "codex-user",
                "mode": "codex-oauth",
                "access-token": "access-token-1234567890",
                "refresh-token": "refresh-token-1234567890"
            }
        ]
    });
    let req = authed_post("/api/dashboard/providers", &token, create_body);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::CREATED, "create failed: {body:?}");

    reload_runtime_config(&harness);

    let req = authed_get("/api/dashboard/providers/Codex%20Gateway", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["upstream"], "codex");
    let auth_profiles = body["auth_profiles"].as_array().unwrap();
    assert_eq!(auth_profiles.len(), 1);
    assert_eq!(
        auth_profiles[0]["qualified_name"],
        "Codex Gateway/codex-user"
    );
    assert_eq!(auth_profiles[0]["mode"], "codex-oauth");
    assert_eq!(auth_profiles[0]["refresh_token_present"], true);

    let config_path = harness.state.config_path.lock().unwrap().clone();
    let raw_contents = std::fs::read_to_string(config_path).unwrap();
    let raw_config = Config::from_yaml_raw(&raw_contents).unwrap();
    assert_eq!(
        raw_config.providers[0].upstream,
        Some(prism_core::provider::UpstreamKind::Codex)
    );
    assert_eq!(raw_config.providers[0].auth_profiles.len(), 1);
    assert_eq!(raw_config.providers[0].api_key, "");
    assert_eq!(raw_config.providers[0].auth_profiles[0].access_token, None);
    assert_eq!(raw_config.providers[0].auth_profiles[0].refresh_token, None);

    let runtime_store = read_auth_runtime_store(&harness);
    let oauth_profiles = runtime_store["oauth_profiles"].as_array().unwrap();
    assert_eq!(oauth_profiles.len(), 1);
    assert_eq!(oauth_profiles[0]["provider"], "Codex Gateway");
    assert_eq!(oauth_profiles[0]["profile_id"], "codex-user");
    assert_eq!(oauth_profiles[0]["access_token"], "access-token-1234567890");
    assert_eq!(
        oauth_profiles[0]["refresh_token"],
        "refresh-token-1234567890"
    );
}

#[tokio::test]
async fn test_list_auth_profiles() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let req = authed_post(
        "/api/dashboard/providers",
        &token,
        json!({
            "format": "openai",
            "name": "auth-profile-openai",
            "auth_profiles": [
                {
                    "id": "billing",
                    "mode": "api-key",
                    "secret": "sk-auth-profile-1234567890abcdef"
                }
            ]
        }),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::CREATED, "create failed: {body:?}");
    reload_runtime_config(&harness);

    let req = authed_get("/api/dashboard/auth-profiles", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    let profiles = body["profiles"].as_array().unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0]["provider"], "auth-profile-openai");
    assert_eq!(profiles[0]["qualified_name"], "auth-profile-openai/billing");
}

#[tokio::test]
async fn test_auth_profiles_runtime_reports_runtime_truth() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let req = authed_get("/api/dashboard/auth-profiles/runtime", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK, "runtime endpoint failed: {body:?}");

    let expected_dir = harness
        .state
        .auth_runtime
        .storage_dir()
        .expect("runtime dir")
        .map(|path| path.display().to_string());
    let expected_auth_file = harness
        .state
        .auth_runtime
        .codex_auth_file_path()
        .expect("runtime auth file")
        .map(|path| path.display().to_string());

    assert_eq!(body["storage_dir"].as_str(), expected_dir.as_deref());
    assert_eq!(
        body["codex_auth_file"].as_str(),
        expected_auth_file.as_deref()
    );
    assert!(body["proxy_url"].is_null());
}

#[tokio::test]
async fn test_connect_anthropic_subscription_profile_persists_runtime_only() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;
    let setup_token = format!("sk-ant-oat01-{}", "a".repeat(96));

    let req = authed_post(
        "/api/dashboard/providers",
        &token,
        json!({
            "format": "claude",
            "name": "anthropic-subscription",
            "base_url": "https://api.anthropic.com",
            "auth_profiles": [
                {
                    "id": "subscription",
                    "mode": "anthropic-claude-subscription"
                }
            ]
        }),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::CREATED, "create failed: {body:?}");
    reload_runtime_config(&harness);

    let req = authed_post(
        "/api/dashboard/auth-profiles/anthropic-subscription/subscription/connect",
        &token,
        json!({
            "secret": setup_token
        }),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK, "connect failed: {body:?}");
    assert_eq!(body["profile"]["mode"], "anthropic-claude-subscription");
    assert_eq!(body["profile"]["connected"], true);
    assert_eq!(body["profile"]["refresh_token_present"], false);
    assert!(body["profile"]["access_token_masked"].is_string());

    reload_runtime_config(&harness);

    let config_path = harness.state.config_path.lock().unwrap().clone();
    let raw_contents = std::fs::read_to_string(config_path).unwrap();
    let raw_config = Config::from_yaml_raw(&raw_contents).unwrap();
    let provider = raw_config
        .providers
        .iter()
        .find(|provider| provider.name == "anthropic-subscription")
        .unwrap();
    assert_eq!(provider.auth_profiles[0].access_token, None);
    assert_eq!(provider.auth_profiles[0].secret, None);

    let runtime_store = read_auth_runtime_store(&harness);
    let oauth_profiles = runtime_store["oauth_profiles"].as_array().unwrap();
    assert_eq!(oauth_profiles.len(), 1);
    assert_eq!(oauth_profiles[0]["provider"], "anthropic-subscription");
    assert_eq!(oauth_profiles[0]["profile_id"], "subscription");
    assert!(
        oauth_profiles[0]["access_token"]
            .as_str()
            .unwrap()
            .starts_with("sk-ant-oat01-")
    );
    assert_eq!(oauth_profiles[0]["refresh_token"], "");
}

#[tokio::test]
async fn test_reject_anthropic_subscription_profile_on_non_official_base_url() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let req = authed_post(
        "/api/dashboard/providers",
        &token,
        json!({
            "format": "claude",
            "name": "anthropic-proxy",
            "base_url": "https://proxy.example.com/anthropic",
            "auth_profiles": [
                {
                    "id": "subscription",
                    "mode": "anthropic-claude-subscription"
                }
            ]
        }),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "expected validation error: {body:?}"
    );
    assert!(
        body["message"]
            .as_str()
            .unwrap_or_default()
            .contains("official https://api.anthropic.com")
    );
}

#[tokio::test]
async fn test_start_codex_oauth() {
    let mock = spawn_mock_codex_oauth_server(json!({
        "access_token": "unused",
        "refresh_token": "unused"
    }))
    .await;
    let harness = create_test_harness_with_auth_runtime(Arc::new(
        prism_server::auth_runtime::AuthRuntimeManager::with_codex_endpoints(
            mock.auth_url.clone(),
            mock.token_url.clone(),
            "test-client".to_string(),
        ),
    ));
    let token = login_and_get_token(&harness).await;

    let req = authed_post(
        "/api/dashboard/providers",
        &token,
        json!({
            "format": "openai",
            "upstream": "codex",
            "name": "codex-start",
            "wire_api": "responses"
        }),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::CREATED, "create failed: {body:?}");
    reload_runtime_config(&harness);

    let req = authed_post(
        "/api/dashboard/auth-profiles/codex/oauth/start",
        &token,
        json!({
            "provider": "codex-start",
            "profile_id": "codex-user",
            "redirect_uri": "http://127.0.0.1:1455/callback"
        }),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK, "oauth start failed: {body:?}");
    assert!(body["state"].as_str().unwrap().len() > 10);
    let auth_url = body["auth_url"].as_str().unwrap();
    assert!(auth_url.starts_with(&mock.auth_url));
    assert!(auth_url.contains("client_id=test-client"));
}

#[tokio::test]
async fn test_complete_codex_oauth_persists_profile() {
    let id_token = fake_id_token("codex@example.com", "acct_123");
    let mock = spawn_mock_codex_oauth_server(json!({
        "access_token": "new-access-token",
        "refresh_token": "new-refresh-token",
        "id_token": id_token,
        "expires_in": 3600
    }))
    .await;
    let harness = create_test_harness_with_auth_runtime(Arc::new(
        prism_server::auth_runtime::AuthRuntimeManager::with_codex_endpoints(
            mock.auth_url.clone(),
            mock.token_url.clone(),
            "test-client".to_string(),
        ),
    ));
    let token = login_and_get_token(&harness).await;

    let req = authed_post(
        "/api/dashboard/providers",
        &token,
        json!({
            "format": "openai",
            "upstream": "codex",
            "name": "codex-complete",
            "wire_api": "responses"
        }),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::CREATED, "create failed: {body:?}");
    reload_runtime_config(&harness);

    let req = authed_post(
        "/api/dashboard/auth-profiles/codex/oauth/start",
        &token,
        json!({
            "provider": "codex-complete",
            "profile_id": "codex-user",
            "redirect_uri": "http://127.0.0.1:1455/callback"
        }),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK, "oauth start failed: {body:?}");
    let state = body["state"].as_str().unwrap().to_string();

    let req = authed_post(
        "/api/dashboard/auth-profiles/codex/oauth/complete",
        &token,
        json!({
            "state": state,
            "code": "oauth-test-code"
        }),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK, "oauth complete failed: {body:?}");
    assert_eq!(
        body["profile"]["qualified_name"],
        "codex-complete/codex-user"
    );
    assert_eq!(body["profile"]["email"], "codex@example.com");
    assert_eq!(body["profile"]["account_id"], "acct_123");

    reload_runtime_config(&harness);

    let config_path = harness.state.config_path.lock().unwrap().clone();
    let raw_contents = std::fs::read_to_string(config_path).unwrap();
    let raw_config = Config::from_yaml_raw(&raw_contents).unwrap();
    let provider = raw_config
        .providers
        .iter()
        .find(|provider| provider.name == "codex-complete")
        .unwrap();
    assert_eq!(provider.api_key, "");
    assert_eq!(provider.auth_profiles.len(), 1);
    let oauth_profile = provider
        .auth_profiles
        .iter()
        .find(|profile| profile.id == "codex-user")
        .unwrap();
    assert_eq!(oauth_profile.access_token, None);
    assert_eq!(oauth_profile.refresh_token, None);

    let runtime_store = read_auth_runtime_store(&harness);
    let oauth_profiles = runtime_store["oauth_profiles"].as_array().unwrap();
    assert_eq!(oauth_profiles.len(), 1);
    assert_eq!(oauth_profiles[0]["access_token"], "new-access-token");
    assert_eq!(oauth_profiles[0]["refresh_token"], "new-refresh-token");
}

#[tokio::test]
async fn test_refresh_codex_oauth_profile() {
    let id_token = fake_id_token("refresh@example.com", "acct_refresh");
    let mock = spawn_mock_codex_oauth_server(json!({
        "access_token": "refreshed-access-token",
        "refresh_token": "refreshed-refresh-token",
        "id_token": id_token,
        "expires_in": 7200
    }))
    .await;
    let harness = create_test_harness_with_auth_runtime(Arc::new(
        prism_server::auth_runtime::AuthRuntimeManager::with_codex_endpoints(
            mock.auth_url.clone(),
            mock.token_url.clone(),
            "test-client".to_string(),
        ),
    ));
    let token = login_and_get_token(&harness).await;

    let req = authed_post(
        "/api/dashboard/providers",
        &token,
        json!({
            "format": "openai",
            "upstream": "codex",
            "name": "codex-refresh",
            "auth_profiles": [
                {
                    "id": "codex-user",
                    "mode": "codex-oauth",
                    "access-token": "stale-access-token",
                    "refresh-token": "stale-refresh-token"
                }
            ]
        }),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::CREATED, "create failed: {body:?}");
    reload_runtime_config(&harness);

    let req = authed_post(
        "/api/dashboard/auth-profiles/codex-refresh/codex-user/refresh",
        &token,
        json!({}),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK, "oauth refresh failed: {body:?}");
    assert_eq!(body["profile"]["email"], "refresh@example.com");
    assert_eq!(body["profile"]["account_id"], "acct_refresh");

    reload_runtime_config(&harness);

    let config_path = harness.state.config_path.lock().unwrap().clone();
    let raw_contents = std::fs::read_to_string(config_path).unwrap();
    let raw_config = Config::from_yaml_raw(&raw_contents).unwrap();
    let provider = raw_config
        .providers
        .iter()
        .find(|provider| provider.name == "codex-refresh")
        .unwrap();
    assert_eq!(provider.auth_profiles[0].access_token, None);
    assert_eq!(provider.auth_profiles[0].refresh_token, None);

    let runtime_store = read_auth_runtime_store(&harness);
    let oauth_profiles = runtime_store["oauth_profiles"].as_array().unwrap();
    assert_eq!(oauth_profiles.len(), 1);
    assert_eq!(oauth_profiles[0]["access_token"], "refreshed-access-token");
    assert_eq!(
        oauth_profiles[0]["refresh_token"],
        "refreshed-refresh-token"
    );
}

#[tokio::test]
async fn test_prepare_auth_serializes_concurrent_codex_refreshes() {
    let mock = spawn_rotating_mock_codex_oauth_server().await;
    let harness = create_test_harness_with_auth_runtime(Arc::new(
        prism_server::auth_runtime::AuthRuntimeManager::with_codex_endpoints(
            mock.auth_url.clone(),
            mock.token_url.clone(),
            "test-client".to_string(),
        ),
    ));
    let token = login_and_get_token(&harness).await;

    let req = authed_post(
        "/api/dashboard/providers",
        &token,
        json!({
            "format": "openai",
            "upstream": "codex",
            "name": "codex-concurrent-refresh",
            "auth_profiles": [
                {
                    "id": "codex-user",
                    "mode": "codex-oauth",
                    "access-token": "stale-access-token",
                    "refresh-token": "stale-refresh-token"
                }
            ]
        }),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::CREATED, "create failed: {body:?}");
    reload_runtime_config(&harness);

    let auth = harness
        .state
        .router
        .credential_map()
        .get("codex-concurrent-refresh")
        .and_then(|entries| entries.first())
        .cloned()
        .expect("expected codex credential");
    let shared = auth
        .oauth_state
        .clone()
        .expect("expected shared oauth state");
    {
        let mut guard = shared.write().expect("oauth state write lock");
        guard.expires_at = Some(chrono::Utc::now() - chrono::Duration::seconds(30));
    }

    let auth_one = auth.clone();
    let auth_two = auth.clone();
    let (first, second) = tokio::join!(
        harness
            .state
            .auth_runtime
            .prepare_auth(&harness.state, &auth_one),
        harness
            .state
            .auth_runtime
            .prepare_auth(&harness.state, &auth_two),
    );
    assert!(first.is_ok(), "first refresh failed: {first:?}");
    assert!(second.is_ok(), "second refresh failed: {second:?}");
    assert_eq!(mock.refresh_requests.load(Ordering::SeqCst), 1);

    let oauth_state = harness
        .state
        .auth_runtime
        .state_for_profile("codex-concurrent-refresh", "codex-user")
        .expect("runtime state lookup failed")
        .expect("missing runtime state");
    assert_eq!(oauth_state.access_token, "refreshed-access-token");
    assert_eq!(oauth_state.refresh_token, "next-refresh-token");
}

#[tokio::test]
async fn test_import_local_codex_auth_profile() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let auth_file = write_codex_auth_file(temp_dir.path(), "local@example.com", "acct_local");
    let harness = create_test_harness_with_auth_runtime(Arc::new(
        prism_server::auth_runtime::AuthRuntimeManager::new().with_codex_auth_file(auth_file),
    ));
    let token = login_and_get_token(&harness).await;

    let req = authed_post(
        "/api/dashboard/providers",
        &token,
        json!({
            "format": "openai",
            "upstream": "codex",
            "name": "codex-local-import",
            "wire_api": "responses"
        }),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::CREATED, "create failed: {body:?}");
    reload_runtime_config(&harness);

    let req = authed_post(
        "/api/dashboard/auth-profiles/codex-local-import/local-user/import-local",
        &token,
        json!({}),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK, "import-local failed: {body:?}");
    assert_eq!(body["profile"]["email"], "local@example.com");
    assert_eq!(body["profile"]["account_id"], "acct_local");
    assert_eq!(body["profile"]["connected"], true);

    let runtime_store = read_auth_runtime_store(&harness);
    let oauth_profiles = runtime_store["oauth_profiles"].as_array().unwrap();
    assert_eq!(oauth_profiles.len(), 1);
    assert_eq!(oauth_profiles[0]["provider"], "codex-local-import");
    assert_eq!(oauth_profiles[0]["profile_id"], "local-user");
    assert_eq!(oauth_profiles[0]["refresh_token"], "local-refresh-token");
}

#[tokio::test]
async fn test_import_local_codex_auth_profile_from_explicit_path() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let auth_file = write_codex_auth_file(temp_dir.path(), "explicit@example.com", "acct_explicit");

    let req = authed_post(
        "/api/dashboard/providers",
        &token,
        json!({
            "format": "openai",
            "upstream": "codex",
            "name": "codex-explicit-import",
            "wire_api": "responses"
        }),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::CREATED, "create failed: {body:?}");
    reload_runtime_config(&harness);

    let req = authed_post(
        "/api/dashboard/auth-profiles/codex-explicit-import/explicit-user/import-local",
        &token,
        json!({
            "path": auth_file.display().to_string(),
        }),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK, "import-local failed: {body:?}");
    assert_eq!(body["profile"]["email"], "explicit@example.com");
    assert_eq!(body["profile"]["account_id"], "acct_explicit");
    assert_eq!(body["profile"]["connected"], true);

    let runtime_store = read_auth_runtime_store(&harness);
    let oauth_profiles = runtime_store["oauth_profiles"].as_array().unwrap();
    assert_eq!(oauth_profiles.len(), 1);
    assert_eq!(oauth_profiles[0]["provider"], "codex-explicit-import");
    assert_eq!(oauth_profiles[0]["profile_id"], "explicit-user");
    assert_eq!(oauth_profiles[0]["refresh_token"], "local-refresh-token");
}

#[tokio::test]
async fn test_codex_device_flow_connects_profile() {
    let id_token = fake_id_token("device@example.com", "acct_device");
    let oauth = spawn_mock_codex_oauth_server(json!({
        "access_token": "device-access-token",
        "refresh_token": "device-refresh-token",
        "id_token": id_token,
        "expires_in": 3600
    }))
    .await;
    let device = spawn_mock_codex_device_server().await;
    let harness = create_test_harness_with_auth_runtime(Arc::new(
        prism_server::auth_runtime::AuthRuntimeManager::with_codex_runtime_endpoints(
            oauth.auth_url.clone(),
            oauth.token_url.clone(),
            "test-client".to_string(),
            device.user_code_url.clone(),
            device.token_url.clone(),
            device.verification_url.clone(),
        ),
    ));
    let token = login_and_get_token(&harness).await;

    let req = authed_post(
        "/api/dashboard/providers",
        &token,
        json!({
            "format": "openai",
            "upstream": "codex",
            "name": "codex-device",
            "wire_api": "responses"
        }),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::CREATED, "create failed: {body:?}");
    reload_runtime_config(&harness);

    let req = authed_post(
        "/api/dashboard/auth-profiles/codex/device/start",
        &token,
        json!({
            "provider": "codex-device",
            "profile_id": "device-user"
        }),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK, "device start failed: {body:?}");
    assert_eq!(body["user_code"], "CODE-ABCD");
    let state = body["state"].as_str().unwrap().to_string();

    let req = authed_post(
        "/api/dashboard/auth-profiles/codex/device/poll",
        &token,
        json!({ "state": state.clone() }),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK, "first device poll failed: {body:?}");
    assert_eq!(body["status"], "pending");

    let req = authed_post(
        "/api/dashboard/auth-profiles/codex/device/poll",
        &token,
        json!({ "state": state }),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "second device poll failed: {body:?}"
    );
    assert_eq!(body["status"], "completed");
    assert_eq!(body["profile"]["email"], "device@example.com");
    assert_eq!(body["profile"]["account_id"], "acct_device");

    let runtime_store = read_auth_runtime_store(&harness);
    let oauth_profiles = runtime_store["oauth_profiles"].as_array().unwrap();
    assert_eq!(oauth_profiles.len(), 1);
    assert_eq!(oauth_profiles[0]["access_token"], "device-access-token");
    assert_eq!(oauth_profiles[0]["refresh_token"], "device-refresh-token");
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
    // Default profile is balanced
    assert_eq!(body["default-profile"], "balanced");
    assert!(body["profiles"]["balanced"].is_object());
}

#[tokio::test]
async fn test_update_routing_default_profile() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    // Update default-profile to stable
    let req = authed_patch(
        "/api/dashboard/routing",
        &token,
        json!({"default-profile": "stable"}),
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
    assert_eq!(body["default-profile"], "stable");
}

#[tokio::test]
async fn test_update_routing_switch_profile() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    // Set to lowest-latency
    let req = authed_patch(
        "/api/dashboard/routing",
        &token,
        json!({"default-profile": "lowest-latency"}),
    );
    let (status, _) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);

    // Reload
    let config_path = harness.state.config_path.lock().unwrap().clone();
    let new_config = Config::load(&config_path).expect("failed to reload config");
    harness.state.config.store(Arc::new(new_config));

    // Switch back to balanced
    let req = authed_patch(
        "/api/dashboard/routing",
        &token,
        json!({"default-profile": "balanced"}),
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
    assert_eq!(body["default-profile"], "balanced");
}

#[tokio::test]
async fn test_update_routing_unknown_fields_ignored() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    // Unknown fields are silently ignored by serde (deny_unknown_fields not set)
    let req = authed_patch(
        "/api/dashboard/routing",
        &token,
        json!({"unknown-field": "value"}),
    );
    let (status, _body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
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
    assert!(body["data"].as_array().unwrap().is_empty());
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
        .log_store
        .push(prism_core::request_record::RequestRecord {
            request_id: "req-1".to_string(),
            timestamp: chrono::Utc::now(),
            method: "POST".to_string(),
            path: "/v1/chat/completions".to_string(),
            stream: false,
            requested_model: Some("gpt-4".to_string()),
            request_body: None,
            upstream_request_body: None,
            provider: Some("openai".to_string()),
            model: Some("gpt-4".to_string()),
            credential_name: None,
            total_attempts: 1,
            status: 200,
            latency_ms: 150,
            response_body: None,
            stream_content_preview: None,
            usage: Some(prism_core::request_record::TokenUsage {
                input_tokens: 100,
                output_tokens: 50,
                ..Default::default()
            }),
            cost: None,
            error: None,
            error_type: None,
            api_key_id: None,
            tenant_id: None,
            client_ip: None,
            client_region: None,
            attempts: vec![],
        })
        .await;
    harness
        .state
        .log_store
        .push(prism_core::request_record::RequestRecord {
            request_id: "req-2".to_string(),
            timestamp: chrono::Utc::now(),
            method: "POST".to_string(),
            path: "/v1/chat/completions".to_string(),
            stream: false,
            requested_model: Some("claude-3".to_string()),
            request_body: None,
            upstream_request_body: None,
            provider: Some("claude".to_string()),
            model: Some("claude-3".to_string()),
            credential_name: None,
            total_attempts: 1,
            status: 500,
            latency_ms: 50,
            response_body: None,
            stream_content_preview: None,
            usage: None,
            cost: None,
            error: Some("Internal Server Error".to_string()),
            error_type: None,
            api_key_id: None,
            tenant_id: None,
            client_ip: None,
            client_region: None,
            attempts: vec![],
        })
        .await;

    let req = authed_get("/api/dashboard/logs/stats", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total_entries"], 2);
    assert_eq!(body["error_count"], 1);
}

#[tokio::test]
async fn test_query_logs_with_entries() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    // Push log entries
    for i in 0..5 {
        harness
            .state
            .log_store
            .push(prism_core::request_record::RequestRecord {
                request_id: format!("req-{i}"),
                timestamp: chrono::Utc::now(),
                method: "POST".to_string(),
                path: "/v1/chat/completions".to_string(),
                stream: false,
                requested_model: Some("gpt-4".to_string()),
                request_body: None,
                upstream_request_body: None,
                provider: Some("openai".to_string()),
                model: Some("gpt-4".to_string()),
                credential_name: None,
                total_attempts: 1,
                status: if i % 2 == 0 { 200 } else { 429 },
                latency_ms: 100,
                response_body: None,
                stream_content_preview: None,
                usage: Some(prism_core::request_record::TokenUsage {
                    input_tokens: 10,
                    output_tokens: 20,
                    ..Default::default()
                }),
                cost: None,
                error: if i % 2 != 0 {
                    Some("rate limited".to_string())
                } else {
                    None
                },
                error_type: None,
                api_key_id: None,
                tenant_id: None,
                client_ip: None,
                client_region: None,
                attempts: vec![],
            })
            .await;
    }

    let req = authed_get("/api/dashboard/logs", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 5);
    let items = body["data"].as_array().unwrap();
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
    assert!(
        ["healthy", "degraded", "unhealthy", "not_configured"]
            .contains(&body["status"].as_str().unwrap())
    );
    assert!(body["uptime_seconds"].is_number());
    assert!(body["host"].is_string());
    assert!(body["port"].is_number());
    assert!(body["providers"].is_array());
}

#[tokio::test]
async fn test_protocol_matrix_includes_endpoint_inventory_and_surface_coverage() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let codex_provider = json!({
        "name": "codex-gateway",
        "format": "openai",
        "upstream": "codex",
        "base_url": "https://chatgpt.com/backend-api/codex",
        "wire_api": "responses",
        "models": ["gpt-5"],
        "auth_profiles": [
            { "id": "codex-user", "mode": "codex-oauth" }
        ],
        "disabled": false
    });
    let claude_provider = json!({
        "name": "claude-gateway",
        "format": "claude",
        "api_key": "sk-ant-test",
        "models": ["claude-sonnet-4-20250514"],
        "disabled": false
    });

    let (status, body) = send_request(
        &harness,
        authed_post("/api/dashboard/providers", &token, codex_provider),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "codex create failed: {body:?}");
    let (status, body) = send_request(
        &harness,
        authed_post("/api/dashboard/providers", &token, claude_provider),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "claude create failed: {body:?}"
    );

    harness.state.provider_probe_cache.insert(
        "codex-gateway".to_string(),
        prism_server::handler::dashboard::providers::ProviderProbeResult {
            provider: "codex-gateway".to_string(),
            upstream: "codex".to_string(),
            status: "warning".to_string(),
            checked_at: chrono::Utc::now().to_rfc3339(),
            latency_ms: 42,
            checks: vec![
                prism_server::handler::dashboard::providers::ProviderProbeCheck {
                    capability: "text".to_string(),
                    status: prism_server::handler::dashboard::providers::ProbeStatus::Verified,
                    message: None,
                },
                prism_server::handler::dashboard::providers::ProviderProbeCheck {
                    capability: "stream".to_string(),
                    status: prism_server::handler::dashboard::providers::ProbeStatus::Verified,
                    message: None,
                },
                prism_server::handler::dashboard::providers::ProviderProbeCheck {
                    capability: "tools".to_string(),
                    status: prism_server::handler::dashboard::providers::ProbeStatus::Verified,
                    message: None,
                },
                prism_server::handler::dashboard::providers::ProviderProbeCheck {
                    capability: "images".to_string(),
                    status: prism_server::handler::dashboard::providers::ProbeStatus::Verified,
                    message: None,
                },
                prism_server::handler::dashboard::providers::ProviderProbeCheck {
                    capability: "json_schema".to_string(),
                    status: prism_server::handler::dashboard::providers::ProbeStatus::Unknown,
                    message: Some("no live probe implemented".to_string()),
                },
                prism_server::handler::dashboard::providers::ProviderProbeCheck {
                    capability: "reasoning".to_string(),
                    status: prism_server::handler::dashboard::providers::ProbeStatus::Unknown,
                    message: Some("no live probe implemented".to_string()),
                },
                prism_server::handler::dashboard::providers::ProviderProbeCheck {
                    capability: "count_tokens".to_string(),
                    status: prism_server::handler::dashboard::providers::ProbeStatus::Unsupported,
                    message: Some("Codex backend does not expose count_tokens".to_string()),
                },
            ],
        },
    );

    let (status, body) = send_request(
        &harness,
        authed_get("/api/dashboard/protocols/matrix", &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "protocol matrix failed: {body:?}");

    let endpoints = body["endpoints"].as_array().expect("endpoints array");
    assert!(
        endpoints
            .iter()
            .any(|entry| entry["path"] == "/v1/responses/ws")
    );
    assert!(
        endpoints
            .iter()
            .any(|entry| entry["path"] == "/api/provider/{provider}/v1/responses/ws")
    );
    assert!(
        endpoints
            .iter()
            .any(|entry| entry["path"] == "/v1/messages/count_tokens")
    );

    let coverage = body["coverage"].as_array().expect("coverage array");
    let codex_ws = coverage
        .iter()
        .find(|entry| {
            entry["provider"] == "codex-gateway" && entry["surface_id"] == "openai_responses_ws"
        })
        .expect("codex ws coverage");
    assert_eq!(codex_ws["state"]["status"], "verified");
    assert_eq!(codex_ws["execution_mode"], "native");

    let claude_responses = coverage
        .iter()
        .find(|entry| {
            entry["provider"] == "claude-gateway" && entry["surface_id"] == "openai_responses"
        })
        .expect("claude responses coverage");
    assert_eq!(claude_responses["state"]["status"], "unsupported");
}

#[tokio::test]
async fn test_provider_capabilities_return_identity_and_extended_probe_truth() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let provider = json!({
        "name": "codex-gateway",
        "format": "openai",
        "upstream": "codex",
        "base_url": "https://chatgpt.com/backend-api/codex",
        "wire_api": "responses",
        "models": ["gpt-5", "gpt-5-mini"],
        "upstream_presentation": {
            "profile": "codex-cli",
            "mode": "always",
            "strict-mode": false,
            "sensitive-words": [],
            "cache-user-id": false,
            "custom-headers": {}
        },
        "auth_profiles": [
            { "id": "codex-user", "mode": "codex-oauth" }
        ],
        "disabled": false
    });

    let (status, body) = send_request(
        &harness,
        authed_post("/api/dashboard/providers", &token, provider),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "provider create failed: {body:?}"
    );

    harness.state.provider_probe_cache.insert(
        "codex-gateway".to_string(),
        prism_server::handler::dashboard::providers::ProviderProbeResult {
            provider: "codex-gateway".to_string(),
            upstream: "codex".to_string(),
            status: "warning".to_string(),
            checked_at: chrono::Utc::now().to_rfc3339(),
            latency_ms: 24,
            checks: vec![
                prism_server::handler::dashboard::providers::ProviderProbeCheck {
                    capability: "text".to_string(),
                    status: prism_server::handler::dashboard::providers::ProbeStatus::Verified,
                    message: None,
                },
                prism_server::handler::dashboard::providers::ProviderProbeCheck {
                    capability: "stream".to_string(),
                    status: prism_server::handler::dashboard::providers::ProbeStatus::Verified,
                    message: None,
                },
                prism_server::handler::dashboard::providers::ProviderProbeCheck {
                    capability: "tools".to_string(),
                    status: prism_server::handler::dashboard::providers::ProbeStatus::Verified,
                    message: None,
                },
                prism_server::handler::dashboard::providers::ProviderProbeCheck {
                    capability: "images".to_string(),
                    status: prism_server::handler::dashboard::providers::ProbeStatus::Verified,
                    message: None,
                },
                prism_server::handler::dashboard::providers::ProviderProbeCheck {
                    capability: "json_schema".to_string(),
                    status: prism_server::handler::dashboard::providers::ProbeStatus::Unknown,
                    message: Some("no live probe implemented".to_string()),
                },
                prism_server::handler::dashboard::providers::ProviderProbeCheck {
                    capability: "reasoning".to_string(),
                    status: prism_server::handler::dashboard::providers::ProbeStatus::Unknown,
                    message: Some("no live probe implemented".to_string()),
                },
                prism_server::handler::dashboard::providers::ProviderProbeCheck {
                    capability: "count_tokens".to_string(),
                    status: prism_server::handler::dashboard::providers::ProbeStatus::Unsupported,
                    message: Some("Codex backend does not expose count_tokens".to_string()),
                },
            ],
        },
    );

    let (status, body) = send_request(
        &harness,
        authed_get("/api/dashboard/providers/capabilities", &token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "capabilities failed: {body:?}");

    let providers = body["providers"].as_array().expect("providers array");
    let codex = providers
        .iter()
        .find(|entry| entry["name"] == "codex-gateway")
        .expect("codex provider");
    assert_eq!(codex["format"], "openai");
    assert_eq!(codex["upstream"], "codex");
    assert_eq!(codex["wire_api"], "responses");
    assert_eq!(codex["presentation_profile"], "codex-cli");
    assert_eq!(codex["probe_status"], "warning");
    assert_eq!(codex["probe"]["count_tokens"]["status"], "unsupported");
    assert!(
        codex["models"]
            .as_array()
            .expect("models array")
            .iter()
            .any(|model| model["id"] == "gpt-5")
    );
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
    // Check sanitized config sections
    assert!(body["listen"].is_object());
    assert!(body["listen"]["host"].is_string());
    assert!(body["listen"]["port"].is_number());
    assert!(body["dashboard"].is_object());
    assert_eq!(body["dashboard"]["enabled"], true);
    assert_eq!(body["dashboard"]["username"], "admin");
    // Password hash and jwt_secret should not be in the sanitized output
    assert!(body["dashboard"]["password_hash"].is_null());
    assert!(body["dashboard"]["jwt_secret"].is_null());
    // Provider summary
    assert!(body["providers"].is_object());
    assert!(body["providers"]["total"].is_number());
    assert!(body["providers"]["items"].is_array());
    // Additional sections
    assert!(body["routing"].is_object());
    assert!(body["auth_keys"].is_object());
    assert!(body["rate_limit"].is_object());
    assert!(body["cache"].is_object());
    assert!(body["retry"].is_object());
    assert!(body["timeouts"].is_object());
    assert!(body["log_store"].is_object());
    // Version hash
    assert!(body["config_version"].is_string());
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
    assert!(body["errors"].is_array());
    assert!(!body["errors"].as_array().unwrap().is_empty());
}

// ===========================================================================
// Token via query parameter tests
// ===========================================================================

#[tokio::test]
async fn test_protected_endpoint_with_token_query_param_rejected() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let uri = format!("/api/dashboard/providers?token={token}");
    let req = Request::builder()
        .method("GET")
        .uri(&uri)
        .body(Body::empty())
        .unwrap();

    let (status, _body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
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
            "format": "openai",
            "api_key": "sk-openai-test-1234567890abcdef",
            "name": "OpenAI Prod"
        }),
        json!({
            "format": "claude",
            "api_key": "sk-ant-claude-test-1234567890abcdef",
            "name": "Claude Prod"
        }),
        json!({
            "format": "gemini",
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

    // Verify provider names
    let names: Vec<&str> = all_providers
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"OpenAI Prod"));
    assert!(names.contains(&"Claude Prod"));
    assert!(names.contains(&"Gemini Prod"));
}

#[tokio::test]
async fn test_providers_with_same_api_key_across_formats() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let shared_key = "sk-sp-shared-test-1234567890abcdef";
    let providers = vec![
        json!({
            "format": "openai",
            "api_key": shared_key,
            "name": "Bailian OpenAI",
            "base_url": "https://coding.dashscope.aliyuncs.com"
        }),
        json!({
            "format": "claude",
            "api_key": shared_key,
            "name": "Bailian Claude",
            "base_url": "https://coding.dashscope.aliyuncs.com/apps/anthropic"
        }),
    ];

    for p in &providers {
        let req = authed_post("/api/dashboard/providers", &token, p.clone());
        let (status, body) = send_request(&harness, req).await;
        assert_eq!(status, StatusCode::CREATED, "create failed: {body:?}");

        let config_path = harness.state.config_path.lock().unwrap().clone();
        let new_config = Config::load(&config_path).expect("failed to reload config");
        harness.state.config.store(Arc::new(new_config));
    }

    let req = authed_get("/api/dashboard/providers", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);

    let all_providers = body["providers"].as_array().unwrap();
    assert_eq!(all_providers.len(), 2, "providers: {all_providers:?}");

    let names: Vec<&str> = all_providers
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"Bailian OpenAI"));
    assert!(names.contains(&"Bailian Claude"));
}

#[tokio::test]
async fn test_fetch_models_returns_unsupported_for_dashscope_coding_plan() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let req = authed_post(
        "/api/dashboard/providers/fetch-models",
        &token,
        json!({
            "format": "openai",
            "upstream": "openai",
            "api_key": "sk-sp-shared-test-1234567890abcdef",
            "base_url": "https://coding.dashscope.aliyuncs.com"
        }),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["supported"], false);
    assert_eq!(body["models"], json!([]));
    assert!(
        body["message"]
            .as_str()
            .unwrap_or_default()
            .contains("configure models manually")
    );
}

#[tokio::test]
async fn test_provider_health_uses_live_text_probe_for_openai_compatible_provider() {
    let probe_server = spawn_mock_openai_probe_server().await;
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let create_body = json!({
        "format": "openai",
        "name": "DashScope Probe",
        "api_key": "sk-sp-shared-test-1234567890abcdef",
        "base_url": probe_server.base_url,
        "models": ["qwen3-coder-plus"]
    });
    let req = authed_post("/api/dashboard/providers", &token, create_body);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::CREATED, "create failed: {body:?}");
    reload_runtime_config(&harness);

    let req = authed_post(
        "/api/dashboard/providers/DashScope%20Probe/health",
        &token,
        json!({}),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK, "health failed: {body:?}");
    assert_eq!(body["status"], "ok");

    let checks = body["checks"].as_array().expect("checks should be array");
    let auth = checks
        .iter()
        .find(|check| check["capability"] == "auth")
        .expect("auth check missing");
    let text = checks
        .iter()
        .find(|check| check["capability"] == "text")
        .expect("text check missing");
    assert_eq!(auth["status"], "verified");
    assert_eq!(text["status"], "verified");
    assert_eq!(probe_server.model_requests.load(Ordering::SeqCst), 0);
    assert_eq!(probe_server.chat_requests.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_provider_test_request_returns_upstream_payload_for_dashboard_operator() {
    let probe_server = spawn_mock_openai_probe_server().await;
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let create_body = json!({
        "format": "openai",
        "name": "DashScope Live Test",
        "api_key": "sk-sp-shared-test-1234567890abcdef",
        "base_url": probe_server.base_url,
        "models": ["qwen3-coder-plus"],
        "wire_api": "responses",
    });
    let req = authed_post("/api/dashboard/providers", &token, create_body);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::CREATED, "create failed: {body:?}");
    reload_runtime_config(&harness);

    let req = authed_post(
        "/api/dashboard/providers/DashScope%20Live%20Test/test-request",
        &token,
        json!({
            "model": "qwen3-coder-plus",
            "input": "Reply with the single word ok.",
        }),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK, "test request failed: {body:?}");
    assert_eq!(body["ok"], true);
    assert_eq!(body["status"], 200);
    assert_eq!(body["model"], "qwen3-coder-plus");
    assert_eq!(
        body["request_body"]["input"],
        "Reply with the single word ok."
    );
    assert_eq!(
        body["response_body"]["output"][0]["content"][0]["text"],
        "ok"
    );
    assert_eq!(probe_server.chat_requests.load(Ordering::SeqCst), 0);
    assert_eq!(probe_server.responses_requests.load(Ordering::SeqCst), 1);
}

// ===========================================================================
// Routing preview/explain tests
// ===========================================================================

#[tokio::test]
async fn test_preview_route_empty_inventory() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let req = authed_post(
        "/api/dashboard/routing/preview",
        &token,
        json!({"model": "gpt-4"}),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK, "preview failed: {body:?}");
    assert_eq!(body["profile"], "balanced");
    // No credentials configured, so no selected route
    assert!(body["selected"].is_null());
    assert!(body["alternates"].as_array().unwrap().is_empty());
    // Preview should not include scoring details
    assert!(body["scoring"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_explain_route_empty_inventory() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let req = authed_post(
        "/api/dashboard/routing/explain",
        &token,
        json!({"model": "gpt-4"}),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK, "explain failed: {body:?}");
    // Explain returns RouteExplanation with scoring details
    assert!(body["selected"].is_null());
    assert!(body["alternates"].as_array().unwrap().is_empty());
    assert!(body["profile"].is_string());
    assert!(body["model_chain"].is_array());
}

#[tokio::test]
async fn test_preview_route_with_providers() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    // Create an OpenAI provider to populate catalog
    let req = authed_post(
        "/api/dashboard/providers",
        &token,
        json!({
            "format": "openai",
            "api_key": "sk-test-preview-1234567890abcdef",
            "name": "Preview Test OpenAI"
        }),
    );
    let (status, _) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::CREATED);

    // Reload config and update catalog
    let config_path = harness.state.config_path.lock().unwrap().clone();
    let new_config = Config::load(&config_path).expect("failed to reload config");
    harness.state.router.update_from_config(&new_config);
    harness
        .state
        .catalog
        .update_from_credentials(&harness.state.router.credential_map());
    harness.state.config.store(Arc::new(new_config));

    // Preview should now find the provider
    let req = authed_post(
        "/api/dashboard/routing/preview",
        &token,
        json!({"model": "gpt-4", "endpoint": "chat-completions"}),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK, "preview failed: {body:?}");
    assert_eq!(body["profile"], "balanced");
    // Model chain should contain the requested model
    assert!(
        body["model_chain"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m == "gpt-4")
    );
}

#[tokio::test]
async fn test_preview_route_invalid_body() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    // Missing required 'model' field
    let req = authed_post(
        "/api/dashboard/routing/preview",
        &token,
        json!({"endpoint": "chat-completions"}),
    );
    let (status, _body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_explain_with_provider() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    // Create a provider
    let req = authed_post(
        "/api/dashboard/providers",
        &token,
        json!({
            "format": "openai",
            "api_key": "sk-test-explain-1234567890abcdef",
            "name": "Explain Test OpenAI"
        }),
    );
    let (status, _) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::CREATED);

    // Reload and update catalog
    let config_path = harness.state.config_path.lock().unwrap().clone();
    let new_config = Config::load(&config_path).expect("failed to reload config");
    harness.state.router.update_from_config(&new_config);
    harness
        .state
        .catalog
        .update_from_credentials(&harness.state.router.credential_map());
    harness.state.config.store(Arc::new(new_config));

    // Explain returns RouteExplanation with full scoring details
    let req = authed_post(
        "/api/dashboard/routing/explain",
        &token,
        json!({"model": "gpt-4"}),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK, "explain failed: {body:?}");
    assert!(body["profile"].is_string());
    assert!(body["model_chain"].is_array());
    assert!(body["alternates"].is_array());
    assert!(body["rejections"].is_array());
    // Explain includes scoring details (not cleared like preview)
    assert!(body["scoring"].is_array());
}

#[tokio::test]
async fn test_update_routing_validation_empty_profiles() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let req = authed_patch("/api/dashboard/routing", &token, json!({"profiles": {}}));
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], "validation_failed");
    let details = body["details"].as_array().unwrap();
    assert!(details.iter().any(|d| {
        d.as_str()
            .unwrap()
            .contains("profiles map must not be empty")
    }));
}

#[tokio::test]
async fn test_update_routing_validation_rule_references_nonexistent_profile() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let req = authed_patch(
        "/api/dashboard/routing",
        &token,
        json!({
            "profiles": {
                "balanced": {
                    "provider-policy": {"strategy": "weighted-round-robin", "weights": {"openai": 100}},
                    "credential-policy": {"strategy": "priority-weighted-rr"},
                    "health": {
                        "circuit-breaker": {"enabled": true, "failure-threshold": 5, "cooldown-seconds": 30},
                        "outlier-detection": {"consecutive-5xx": 5, "consecutive-local-failures": 3, "base-eject-seconds": 10, "max-eject-seconds": 300}
                    },
                    "failover": {"credential-attempts": 2, "provider-attempts": 2, "model-attempts": 2, "retry-budget": {"ratio": 0.2, "min-retries-per-second": 1}, "retry-on": ["429", "5xx"]}
                }
            },
            "rules": [
                {"name": "test-rule", "match": {"models": ["gpt-*"]}, "use-profile": "nonexistent"}
            ]
        }),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], "validation_failed");
    let details = body["details"].as_array().unwrap();
    assert!(
        details
            .iter()
            .any(|d| d.as_str().unwrap().contains("nonexistent"))
    );
}

#[tokio::test]
async fn test_update_routing_then_preview_reflects_change() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    // Switch to stable profile
    let req = authed_patch(
        "/api/dashboard/routing",
        &token,
        json!({"default-profile": "stable"}),
    );
    let (status, _) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);

    // Reload config
    let config_path = harness.state.config_path.lock().unwrap().clone();
    let new_config = Config::load(&config_path).expect("failed to reload config");
    harness.state.config.store(Arc::new(new_config));

    // Preview should reflect the new profile
    let req = authed_post(
        "/api/dashboard/routing/preview",
        &token,
        json!({"model": "gpt-4"}),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["profile"], "stable");
}

#[tokio::test]
async fn test_preview_route_accepts_routing_override() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let mut override_routing: RoutingConfig = harness.state.config.load().routing.clone();
    override_routing.default_profile = "stable".to_string();

    let req = authed_post(
        "/api/dashboard/routing/preview",
        &token,
        json!({
            "model": "gpt-4",
            "routing_override": serde_json::to_value(&override_routing).expect("serialize routing override"),
        }),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "preview with override failed: {body:?}"
    );
    assert_eq!(body["profile"], "stable");
}

#[tokio::test]
async fn test_preview_route_rejects_invalid_routing_override() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let mut override_routing: RoutingConfig = harness.state.config.load().routing.clone();
    override_routing.default_profile = "missing-profile".to_string();

    let req = authed_post(
        "/api/dashboard/routing/preview",
        &token,
        json!({
            "model": "gpt-4",
            "routing_override": serde_json::to_value(&override_routing).expect("serialize routing override"),
        }),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], "validation_failed");
    assert!(body["details"].as_array().unwrap().iter().any(|detail| {
        detail
            .as_str()
            .unwrap_or_default()
            .contains("default-profile")
    }));
}

// ===========================================================================
// Config version tracking tests (#259 / #262)
// ===========================================================================

#[tokio::test]
async fn test_raw_config_returns_version() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let req = authed_get("/api/dashboard/config/raw", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["content"].is_string());
    assert!(body["path"].is_string());
    let version = body["config_version"]
        .as_str()
        .expect("should have config_version");
    assert!(!version.is_empty(), "config_version should be non-empty");
}

#[tokio::test]
async fn test_current_config_returns_version() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let req = authed_get("/api/dashboard/config/current", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    let version = body["config_version"]
        .as_str()
        .expect("should have config_version");
    assert!(!version.is_empty());
}

#[tokio::test]
async fn test_apply_config_success_with_version() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    // Get current raw config and its version
    let req = authed_get("/api/dashboard/config/raw", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    let yaml_content = body["content"].as_str().unwrap().to_string();
    let version = body["config_version"].as_str().unwrap().to_string();

    // Apply with matching version — should succeed
    let req = authed_put(
        "/api/dashboard/config/apply",
        &token,
        json!({"yaml": yaml_content, "config_version": version}),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK, "apply failed: {body:?}");
    assert!(body["config_version"].is_string());
}

#[tokio::test]
async fn test_apply_config_conflict_detection() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    // Get current raw config
    let req = authed_get("/api/dashboard/config/raw", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    let yaml_content = body["content"].as_str().unwrap().to_string();

    // Apply with stale version — should return 409 Conflict
    let req = authed_put(
        "/api/dashboard/config/apply",
        &token,
        json!({"yaml": yaml_content, "config_version": "stale-version-hash"}),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::CONFLICT, "expected conflict: {body:?}");
    assert_eq!(body["error"], "config_conflict");
    assert!(body["current_version"].is_string());
}

#[tokio::test]
async fn test_apply_config_without_version_skips_conflict_check() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    // Get current raw config
    let req = authed_get("/api/dashboard/config/raw", &token);
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK);
    let yaml_content = body["content"].as_str().unwrap().to_string();

    // Apply without version — no conflict check, should succeed
    let req = authed_put(
        "/api/dashboard/config/apply",
        &token,
        json!({"yaml": yaml_content}),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK, "apply failed: {body:?}");
    assert!(body["config_version"].is_string());
}

#[tokio::test]
async fn test_apply_config_invalid_yaml() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let req = authed_put(
        "/api/dashboard/config/apply",
        &token,
        json!({"yaml": "port: not-a-number\n"}),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], "validation_failed");
}

#[tokio::test]
async fn test_apply_config_missing_yaml_field() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let req = authed_put(
        "/api/dashboard/config/apply",
        &token,
        json!({"content": "some yaml"}),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], "validation_failed");
}

#[tokio::test]
async fn test_routing_update_returns_version() {
    let harness = create_test_harness();
    let token = login_and_get_token(&harness).await;

    let req = authed_patch(
        "/api/dashboard/routing",
        &token,
        json!({"default-profile": "stable"}),
    );
    let (status, body) = send_request(&harness, req).await;
    assert_eq!(status, StatusCode::OK, "update routing failed: {body:?}");
    // Routing update now returns config_version
    assert!(body["config_version"].is_string());
}

fn write_test_config(harness: &TestHarness, config: &Config) {
    let path = harness.state.config_path.lock().unwrap().clone();
    std::fs::write(&path, config.to_yaml().expect("serialize config")).expect("write test config");
    reload_runtime_config(harness);
}

struct ProviderFixture<'a> {
    name: &'a str,
    format: Format,
    upstream: Option<UpstreamKind>,
    wire_api: WireApi,
    models: &'a [&'a str],
    auth_profiles: Vec<AuthProfileEntry>,
    api_key: &'a str,
    base_url: Option<&'a str>,
    region: Option<&'a str>,
}

fn provider_entry(fixture: ProviderFixture<'_>) -> prism_core::config::ProviderKeyEntry {
    prism_core::config::ProviderKeyEntry {
        name: fixture.name.to_string(),
        format: fixture.format,
        upstream: fixture.upstream,
        api_key: fixture.api_key.to_string(),
        base_url: fixture.base_url.map(str::to_string),
        proxy_url: None,
        prefix: None,
        models: fixture
            .models
            .iter()
            .map(|model| prism_core::config::ModelMapping {
                id: (*model).to_string(),
                alias: None,
            })
            .collect(),
        excluded_models: Vec::new(),
        headers: HashMap::new(),
        disabled: false,
        cloak: Default::default(),
        wire_api: fixture.wire_api,
        weight: 1,
        region: fixture.region.map(str::to_string),
        credential_source: None,
        auth_profiles: fixture.auth_profiles,
        upstream_presentation: Default::default(),
        vertex: false,
        vertex_project: None,
        vertex_location: None,
    }
}

async fn seed_control_plane_fixture(harness: &TestHarness) {
    let mut config = harness.state.config.load().as_ref().clone();
    config.providers = vec![
        provider_entry(ProviderFixture {
            name: "claude-sub-eu",
            format: Format::Claude,
            upstream: Some(UpstreamKind::Claude),
            wire_api: WireApi::Chat,
            models: &["claude-3-7-sonnet", "claude-3-5-haiku"],
            auth_profiles: vec![AuthProfileEntry {
                id: "subscription".to_string(),
                mode: AuthMode::AnthropicClaudeSubscription,
                header: prism_core::auth_profile::AuthHeaderKind::Auto,
                region: Some("eu-central".to_string()),
                ..Default::default()
            }],
            api_key: "",
            base_url: Some("https://api.anthropic.com"),
            region: Some("eu-central"),
        }),
        provider_entry(ProviderFixture {
            name: "openai-prod",
            format: Format::OpenAI,
            upstream: Some(UpstreamKind::OpenAI),
            wire_api: WireApi::Responses,
            models: &["gpt-5-mini", "gpt-4.1-mini"],
            auth_profiles: Vec::new(),
            api_key: "sk-openai-prod-1234567890",
            base_url: Some("https://api.openai.com"),
            region: Some("us-east"),
        }),
    ];
    config.auth_keys = vec![
        AuthKeyEntry {
            key: "sk-proxy-tenant-red".to_string(),
            name: Some("tenant-red".to_string()),
            tenant_id: Some("tenant-red".to_string()),
            allowed_models: vec!["claude-*".to_string(), "gpt-*".to_string()],
            allowed_credentials: Vec::new(),
            rate_limit: None,
            budget: None,
            expires_at: None,
            metadata: HashMap::new(),
        },
        AuthKeyEntry {
            key: "sk-proxy-tenant-blue".to_string(),
            name: Some("tenant-blue".to_string()),
            tenant_id: Some("tenant-blue".to_string()),
            allowed_models: vec!["*".to_string()],
            allowed_credentials: Vec::new(),
            rate_limit: None,
            budget: None,
            expires_at: None,
            metadata: HashMap::new(),
        },
    ];
    config.routing.rules = vec![RouteRule {
        name: "tenant-red-claude".to_string(),
        priority: Some(100),
        match_conditions: RouteMatch {
            models: vec!["claude-*".to_string()],
            tenants: vec!["tenant-red".to_string()],
            endpoints: vec!["chat-completions".to_string()],
            ..Default::default()
        },
        use_profile: "balanced".to_string(),
    }];
    write_test_config(harness, &config);

    harness.state.provider_probe_cache.insert(
        "claude-sub-eu".to_string(),
        prism_server::handler::dashboard::providers::ProviderProbeResult {
            provider: "claude-sub-eu".to_string(),
            upstream: "claude".to_string(),
            status: "failed".to_string(),
            checked_at: Utc::now().to_rfc3339(),
            latency_ms: 810,
            checks: vec![
                prism_server::handler::dashboard::providers::ProviderProbeCheck {
                    capability: "text".to_string(),
                    status: prism_server::handler::dashboard::providers::ProbeStatus::Failed,
                    message: Some("managed auth disconnected".to_string()),
                },
                prism_server::handler::dashboard::providers::ProviderProbeCheck {
                    capability: "stream".to_string(),
                    status: prism_server::handler::dashboard::providers::ProbeStatus::Unknown,
                    message: Some("not probed".to_string()),
                },
            ],
        },
    );
    harness.state.provider_probe_cache.insert(
        "openai-prod".to_string(),
        prism_server::handler::dashboard::providers::ProviderProbeResult {
            provider: "openai-prod".to_string(),
            upstream: "openai".to_string(),
            status: "verified".to_string(),
            checked_at: Utc::now().to_rfc3339(),
            latency_ms: 211,
            checks: vec![
                prism_server::handler::dashboard::providers::ProviderProbeCheck {
                    capability: "text".to_string(),
                    status: prism_server::handler::dashboard::providers::ProbeStatus::Verified,
                    message: None,
                },
                prism_server::handler::dashboard::providers::ProviderProbeCheck {
                    capability: "stream".to_string(),
                    status: prism_server::handler::dashboard::providers::ProbeStatus::Verified,
                    message: None,
                },
            ],
        },
    );

    harness
        .state
        .log_store
        .push(RequestRecord {
            request_id: "req_traffic_latest".to_string(),
            timestamp: Utc::now(),
            method: "POST".to_string(),
            path: "/v1/chat/completions".to_string(),
            stream: false,
            requested_model: Some("claude-3-7-sonnet".to_string()),
            request_body: None,
            upstream_request_body: None,
            provider: Some("openai-prod".to_string()),
            model: Some("gpt-5-mini".to_string()),
            credential_name: Some("openai-prod".to_string()),
            total_attempts: 2,
            status: 200,
            latency_ms: 1840,
            response_body: None,
            stream_content_preview: None,
            usage: Some(TokenUsage {
                input_tokens: 1200,
                output_tokens: 410,
                cache_read_tokens: 0,
                cache_creation_tokens: 0,
            }),
            cost: Some(0.0021),
            error: None,
            error_type: None,
            api_key_id: Some("sk-p****-red".to_string()),
            tenant_id: Some("tenant-red".to_string()),
            client_ip: Some("127.0.0.1".to_string()),
            client_region: Some("eu-central".to_string()),
            attempts: vec![
                AttemptSummary {
                    attempt_index: 0,
                    provider: "claude-sub-eu".to_string(),
                    model: "claude-3-7-sonnet".to_string(),
                    credential_name: Some("subscription".to_string()),
                    status: Some(401),
                    latency_ms: 410,
                    error: Some("managed auth disconnected".to_string()),
                    error_type: Some("auth_runtime".to_string()),
                },
                AttemptSummary {
                    attempt_index: 1,
                    provider: "openai-prod".to_string(),
                    model: "gpt-5-mini".to_string(),
                    credential_name: Some("openai-prod".to_string()),
                    status: Some(200),
                    latency_ms: 1430,
                    error: None,
                    error_type: None,
                },
            ],
        })
        .await;
    harness
        .state
        .log_store
        .push(RequestRecord {
            request_id: "req_provider_fail".to_string(),
            timestamp: Utc::now() - ChronoDuration::minutes(10),
            method: "POST".to_string(),
            path: "/v1/messages".to_string(),
            stream: false,
            requested_model: Some("claude-3-5-haiku".to_string()),
            request_body: None,
            upstream_request_body: None,
            provider: Some("claude-sub-eu".to_string()),
            model: Some("claude-3-5-haiku".to_string()),
            credential_name: Some("subscription".to_string()),
            total_attempts: 1,
            status: 503,
            latency_ms: 920,
            response_body: None,
            stream_content_preview: None,
            usage: None,
            cost: None,
            error: Some("upstream unavailable".to_string()),
            error_type: Some("upstream_5xx".to_string()),
            api_key_id: Some("sk-p****-red".to_string()),
            tenant_id: Some("tenant-red".to_string()),
            client_ip: Some("127.0.0.1".to_string()),
            client_region: Some("eu-central".to_string()),
            attempts: vec![AttemptSummary {
                attempt_index: 0,
                provider: "claude-sub-eu".to_string(),
                model: "claude-3-5-haiku".to_string(),
                credential_name: Some("subscription".to_string()),
                status: Some(503),
                latency_ms: 920,
                error: Some("upstream unavailable".to_string()),
                error_type: Some("upstream_5xx".to_string()),
            }],
        })
        .await;
    harness
        .state
        .log_store
        .push(RequestRecord {
            request_id: "req_openai_ok".to_string(),
            timestamp: Utc::now() - ChronoDuration::minutes(20),
            method: "POST".to_string(),
            path: "/v1/responses".to_string(),
            stream: false,
            requested_model: Some("gpt-5-mini".to_string()),
            request_body: None,
            upstream_request_body: None,
            provider: Some("openai-prod".to_string()),
            model: Some("gpt-5-mini".to_string()),
            credential_name: Some("openai-prod".to_string()),
            total_attempts: 1,
            status: 200,
            latency_ms: 610,
            response_body: None,
            stream_content_preview: None,
            usage: Some(TokenUsage {
                input_tokens: 800,
                output_tokens: 260,
                cache_read_tokens: 0,
                cache_creation_tokens: 0,
            }),
            cost: Some(0.0011),
            error: None,
            error_type: None,
            api_key_id: Some("sk-p****-blue".to_string()),
            tenant_id: Some("tenant-blue".to_string()),
            client_ip: Some("127.0.0.1".to_string()),
            client_region: Some("us-east".to_string()),
            attempts: vec![AttemptSummary {
                attempt_index: 0,
                provider: "openai-prod".to_string(),
                model: "gpt-5-mini".to_string(),
                credential_name: Some("openai-prod".to_string()),
                status: Some(200),
                latency_ms: 610,
                error: None,
                error_type: None,
            }],
        })
        .await;
}

#[tokio::test]
async fn test_control_plane_command_center_workspace_endpoint() {
    let harness = create_test_harness();
    seed_control_plane_fixture(&harness).await;
    let token = login_and_get_token(&harness).await;

    let req = authed_get(
        "/api/dashboard/control-plane/command-center?range=1h&source_mode=hybrid",
        &token,
    );
    let (status, body) = send_request(&harness, req).await;

    assert_eq!(status, StatusCode::OK, "command center failed: {body:?}");
    assert!(
        body["kpis"]
            .as_array()
            .is_some_and(|items| items.len() >= 4)
    );
    assert!(
        body["signals"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    );
    assert_eq!(
        body["inspector"]["eyebrow"]["key"],
        "commandCenter.inspector.eyebrow"
    );
}

#[tokio::test]
async fn test_control_plane_traffic_lab_workspace_endpoint() {
    let harness = create_test_harness();
    seed_control_plane_fixture(&harness).await;
    let token = login_and_get_token(&harness).await;

    let req = authed_get(
        "/api/dashboard/control-plane/traffic-lab?range=1h&source_mode=hybrid&limit=5",
        &token,
    );
    let (status, body) = send_request(&harness, req).await;

    assert_eq!(status, StatusCode::OK, "traffic lab failed: {body:?}");
    assert_eq!(body["selected_request_id"], "req_traffic_latest");
    assert!(
        body["sessions"]
            .as_array()
            .is_some_and(|items| items.len() >= 3)
    );
    assert!(
        body["trace"]
            .as_array()
            .is_some_and(|items| items.len() >= 3)
    );
}

#[tokio::test]
async fn test_control_plane_provider_atlas_workspace_endpoint() {
    let harness = create_test_harness();
    seed_control_plane_fixture(&harness).await;
    let token = login_and_get_token(&harness).await;

    let req = authed_get(
        "/api/dashboard/control-plane/provider-atlas?range=1h&source_mode=runtime",
        &token,
    );
    let (status, body) = send_request(&harness, req).await;

    assert_eq!(status, StatusCode::OK, "provider atlas failed: {body:?}");
    assert!(
        body["providers"]
            .as_array()
            .is_some_and(|items| items.len() == 2)
    );
    assert!(
        body["coverage"]
            .as_array()
            .is_some_and(|items| items.len() >= 3)
    );
    assert_eq!(body["providers"][0]["provider"], "claude-sub-eu");
}

#[tokio::test]
async fn test_control_plane_route_studio_workspace_endpoint() {
    let harness = create_test_harness();
    seed_control_plane_fixture(&harness).await;
    let token = login_and_get_token(&harness).await;

    let req = authed_get(
        "/api/dashboard/control-plane/route-studio?range=1h&source_mode=hybrid",
        &token,
    );
    let (status, body) = send_request(&harness, req).await;

    assert_eq!(status, StatusCode::OK, "route studio failed: {body:?}");
    assert!(
        body["summary_facts"]
            .as_array()
            .is_some_and(|items| items.len() >= 4)
    );
    assert!(
        body["scenarios"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    );
    assert_eq!(
        body["inspector"]["eyebrow"]["key"],
        "routeStudio.inspector.eyebrow"
    );
}

#[tokio::test]
async fn test_control_plane_change_studio_workspace_endpoint() {
    let harness = create_test_harness();
    seed_control_plane_fixture(&harness).await;
    let token = login_and_get_token(&harness).await;

    let req = authed_get(
        "/api/dashboard/control-plane/change-studio?range=24h&source_mode=runtime",
        &token,
    );
    let (status, body) = send_request(&harness, req).await;

    assert_eq!(status, StatusCode::OK, "change studio failed: {body:?}");
    assert!(
        body["registry"]
            .as_array()
            .is_some_and(|items| items.len() >= 5)
    );
    assert!(
        body["publish_facts"]
            .as_array()
            .is_some_and(|items| items.len() >= 4)
    );
    assert_eq!(
        body["inspector"]["eyebrow"]["key"],
        "changeStudio.inspector.eyebrow"
    );
}
