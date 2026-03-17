pub mod app;
pub mod auth;
pub mod auth_runtime;
pub mod dispatch;
pub mod handler;
pub mod middleware;
pub mod streaming;
pub mod telemetry;

use arc_swap::ArcSwap;
use axum::{Router, middleware as axum_mw};
use prism_core::cache::ResponseCacheBackend;
use prism_core::config::Config;
use prism_core::cost::CostCalculator;
use prism_core::metrics::Metrics;
use prism_core::rate_limit::CompositeRateLimiter;
use prism_core::request_log::LogStore;
use prism_core::thinking_cache::ThinkingCache;
use prism_provider::ExecutorRegistry;
use prism_provider::catalog::ProviderCatalog;
use prism_provider::health::HealthManager;
use prism_provider::routing::CredentialRouter;
use prism_translator::TranslatorRegistry;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<ArcSwap<Config>>,
    pub router: Arc<CredentialRouter>,
    pub executors: Arc<ExecutorRegistry>,
    pub translators: Arc<TranslatorRegistry>,
    pub metrics: Arc<Metrics>,
    pub log_store: Arc<dyn LogStore>,
    pub config_path: Arc<Mutex<String>>,
    pub rate_limiter: Arc<CompositeRateLimiter>,
    pub cost_calculator: Arc<CostCalculator>,
    pub response_cache: Option<Arc<dyn ResponseCacheBackend>>,
    pub http_client_pool: Arc<prism_core::proxy::HttpClientPool>,
    pub thinking_cache: Option<Arc<ThinkingCache>>,
    pub start_time: Instant,
    pub login_limiter: Arc<handler::dashboard::auth::LoginRateLimiter>,
    pub catalog: Arc<ProviderCatalog>,
    pub health_manager: Arc<HealthManager>,
    pub auth_runtime: Arc<auth_runtime::AuthRuntimeManager>,
    pub oauth_sessions: Arc<dashmap::DashMap<String, auth_runtime::PendingCodexOauthSession>>,
    pub device_sessions: Arc<dashmap::DashMap<String, auth_runtime::PendingCodexDeviceSession>>,
    pub provider_probe_cache:
        Arc<dashmap::DashMap<String, handler::dashboard::providers::ProviderProbeResult>>,
}

pub fn build_router(state: AppState) -> Router {
    let body_limit_bytes = state.config.load().body_limit_mb * 1024 * 1024;

    // Public routes — no auth required
    let public_routes = Router::new()
        .route("/health", axum::routing::get(handler::health::health))
        .route("/metrics", axum::routing::get(handler::health::metrics))
        .route(
            "/metrics/prometheus",
            axum::routing::get(handler::health::prometheus_metrics),
        );

    // Admin routes — no auth required (read-only)
    let admin_routes = Router::new()
        .route(
            "/admin/config",
            axum::routing::get(handler::admin::admin_config),
        )
        .route(
            "/admin/metrics",
            axum::routing::get(handler::admin::admin_metrics),
        )
        .route(
            "/admin/models",
            axum::routing::get(handler::admin::admin_models),
        );

    // API routes — auth required, with body size limit
    let api_routes = Router::new()
        .route(
            "/v1/models",
            axum::routing::get(handler::models::list_models),
        )
        .route(
            "/v1/chat/completions",
            axum::routing::post(handler::chat_completions::chat_completions),
        )
        .route(
            "/v1/messages",
            axum::routing::post(handler::messages::messages),
        )
        .route(
            "/v1/completions",
            axum::routing::post(handler::completions::completions),
        )
        .route(
            "/v1/responses",
            axum::routing::post(handler::responses::responses),
        )
        .route(
            "/v1/responses/ws",
            axum::routing::get(handler::responses_ws::responses_ws),
        )
        .route(
            "/v1/messages/count_tokens",
            axum::routing::post(handler::count_tokens::count_tokens),
        )
        // Gemini native routes
        .route(
            "/v1beta/models",
            axum::routing::get(handler::gemini::list_models),
        )
        .route(
            "/v1beta/models/{model_action}",
            axum::routing::post(handler::gemini::gemini_model_action),
        )
        // Provider-scoped routes
        .route(
            "/api/provider/{provider}/v1/chat/completions",
            axum::routing::post(handler::provider_scoped::provider_chat_completions),
        )
        .route(
            "/api/provider/{provider}/v1/messages",
            axum::routing::post(handler::provider_scoped::provider_messages),
        )
        .route(
            "/api/provider/{provider}/v1/responses",
            axum::routing::post(handler::provider_scoped::provider_responses),
        )
        .route(
            "/api/provider/{provider}/v1/responses/ws",
            axum::routing::get(handler::responses_ws::provider_responses_ws),
        )
        .layer(RequestBodyLimitLayer::new(body_limit_bytes))
        .layer(axum_mw::from_fn_with_state(
            state.clone(),
            middleware::rate_limit::rate_limit_middleware,
        ))
        .layer(axum_mw::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ));

    // Dashboard auth routes — no auth required (login endpoint)
    let dashboard_auth_routes = Router::new()
        .route(
            "/api/dashboard/auth/login",
            axum::routing::post(handler::dashboard::auth::login),
        )
        .route(
            "/api/dashboard/auth/session",
            axum::routing::get(handler::dashboard::auth::session),
        );

    // Dashboard protected routes — JWT auth required
    let dashboard_protected_routes = Router::new()
        .route(
            "/api/dashboard/auth/refresh",
            axum::routing::post(handler::dashboard::auth::refresh),
        )
        .route(
            "/api/dashboard/auth/logout",
            axum::routing::post(handler::dashboard::auth::logout),
        )
        .route(
            "/api/dashboard/auth-profiles",
            axum::routing::get(handler::dashboard::auth_profiles::list_auth_profiles)
                .post(handler::dashboard::auth_profiles::create_auth_profile),
        )
        .route(
            "/api/dashboard/auth-profiles/runtime",
            axum::routing::get(handler::dashboard::auth_profiles::auth_profiles_runtime),
        )
        .route(
            "/api/dashboard/auth-profiles/{provider}/{profile}",
            axum::routing::put(handler::dashboard::auth_profiles::replace_auth_profile)
                .delete(handler::dashboard::auth_profiles::delete_auth_profile),
        )
        .route(
            "/api/dashboard/auth-profiles/codex/oauth/start",
            axum::routing::post(handler::dashboard::auth_profiles::start_codex_oauth),
        )
        .route(
            "/api/dashboard/auth-profiles/codex/oauth/complete",
            axum::routing::post(handler::dashboard::auth_profiles::complete_codex_oauth),
        )
        .route(
            "/api/dashboard/auth-profiles/codex/device/start",
            axum::routing::post(handler::dashboard::auth_profiles::start_codex_device),
        )
        .route(
            "/api/dashboard/auth-profiles/codex/device/poll",
            axum::routing::post(handler::dashboard::auth_profiles::poll_codex_device),
        )
        .route(
            "/api/dashboard/auth-profiles/{provider}/{profile}/connect",
            axum::routing::post(handler::dashboard::auth_profiles::connect_auth_profile),
        )
        .route(
            "/api/dashboard/auth-profiles/{provider}/{profile}/import-local",
            axum::routing::post(handler::dashboard::auth_profiles::import_local_auth_profile),
        )
        .route(
            "/api/dashboard/auth-profiles/{provider}/{profile}/refresh",
            axum::routing::post(handler::dashboard::auth_profiles::refresh_auth_profile),
        )
        // Providers
        .route(
            "/api/dashboard/providers/fetch-models",
            axum::routing::post(handler::dashboard::providers::fetch_models),
        )
        .route(
            "/api/dashboard/providers/{id}/health",
            axum::routing::post(handler::dashboard::providers::health_check),
        )
        .route(
            "/api/dashboard/providers/{id}/presentation-preview",
            axum::routing::post(handler::dashboard::providers::presentation_preview),
        )
        .route(
            "/api/dashboard/providers",
            axum::routing::get(handler::dashboard::providers::list_providers)
                .post(handler::dashboard::providers::create_provider),
        )
        .route(
            "/api/dashboard/providers/{id}",
            axum::routing::get(handler::dashboard::providers::get_provider)
                .patch(handler::dashboard::providers::update_provider)
                .delete(handler::dashboard::providers::delete_provider),
        )
        // Auth keys
        .route(
            "/api/dashboard/auth-keys",
            axum::routing::get(handler::dashboard::auth_keys::list_auth_keys)
                .post(handler::dashboard::auth_keys::create_auth_key),
        )
        .route(
            "/api/dashboard/auth-keys/{id}",
            axum::routing::patch(handler::dashboard::auth_keys::update_auth_key)
                .delete(handler::dashboard::auth_keys::delete_auth_key),
        )
        .route(
            "/api/dashboard/auth-keys/{id}/reveal",
            axum::routing::post(handler::dashboard::auth_keys::reveal_auth_key),
        )
        // Routing
        .route(
            "/api/dashboard/routing",
            axum::routing::get(handler::dashboard::routing::get_routing)
                .patch(handler::dashboard::routing::update_routing),
        )
        .route(
            "/api/dashboard/routing/preview",
            axum::routing::post(handler::dashboard::routing::preview_route),
        )
        // Config operations
        .route(
            "/api/dashboard/config/validate",
            axum::routing::post(handler::dashboard::config_ops::validate_config),
        )
        .route(
            "/api/dashboard/config/reload",
            axum::routing::post(handler::dashboard::config_ops::reload_config),
        )
        .route(
            "/api/dashboard/config/apply",
            axum::routing::put(handler::dashboard::config_ops::apply_config),
        )
        .route(
            "/api/dashboard/config/current",
            axum::routing::get(handler::dashboard::config_ops::get_config),
        )
        .route(
            "/api/dashboard/config/raw",
            axum::routing::get(handler::dashboard::config_ops::get_raw_config),
        )
        // Request logs — filters before {id} to avoid capture
        .route(
            "/api/dashboard/logs/stats",
            axum::routing::get(handler::dashboard::logs::log_stats),
        )
        .route(
            "/api/dashboard/logs/filters",
            axum::routing::get(handler::dashboard::logs::filter_options),
        )
        .route(
            "/api/dashboard/logs/{id}",
            axum::routing::get(handler::dashboard::logs::get_log),
        )
        .route(
            "/api/dashboard/logs",
            axum::routing::get(handler::dashboard::logs::query_logs),
        )
        // System
        .route(
            "/api/dashboard/system/health",
            axum::routing::get(handler::dashboard::system::system_health),
        )
        .route(
            "/api/dashboard/system/logs",
            axum::routing::get(handler::dashboard::system::system_logs),
        )
        // Tenants
        .route(
            "/api/dashboard/tenants",
            axum::routing::get(handler::dashboard::tenant::list_tenants),
        )
        .route(
            "/api/dashboard/tenants/{id}/metrics",
            axum::routing::get(handler::dashboard::tenant::tenant_metrics),
        )
        // Control Plane (SPEC-065)
        .route(
            "/api/dashboard/protocols/matrix",
            axum::routing::get(handler::dashboard::control_plane::protocol_matrix),
        )
        .route(
            "/api/dashboard/providers/capabilities",
            axum::routing::get(handler::dashboard::control_plane::provider_capabilities),
        )
        .route(
            "/api/dashboard/control-plane/command-center",
            axum::routing::get(handler::dashboard::control_plane_workspace::command_center),
        )
        .route(
            "/api/dashboard/control-plane/traffic-lab",
            axum::routing::get(handler::dashboard::control_plane_workspace::traffic_lab),
        )
        .route(
            "/api/dashboard/control-plane/provider-atlas",
            axum::routing::get(handler::dashboard::control_plane_workspace::provider_atlas),
        )
        .route(
            "/api/dashboard/control-plane/route-studio",
            axum::routing::get(handler::dashboard::control_plane_workspace::route_studio),
        )
        .route(
            "/api/dashboard/control-plane/change-studio",
            axum::routing::get(handler::dashboard::control_plane_workspace::change_studio),
        )
        .route(
            "/api/dashboard/routing/explain",
            axum::routing::post(handler::dashboard::routing::explain_route),
        )
        // WebSocket route (auth via bearer header or session cookie)
        .route(
            "/ws/dashboard",
            axum::routing::get(handler::dashboard::websocket::ws_handler),
        )
        .layer(axum_mw::from_fn_with_state(
            state.clone(),
            middleware::dashboard_auth::dashboard_auth_middleware,
        ))
        // Dashboard body size limit (1 MB) to reject oversized payloads
        .layer(RequestBodyLimitLayer::new(1024 * 1024));

    // Compose: public + admin + api + dashboard, then global middleware layers (outer → inner)
    let mut router = Router::new()
        .merge(public_routes)
        .merge(admin_routes)
        .merge(api_routes);

    // Only register dashboard routes when dashboard is enabled
    if state.config.load().dashboard.enabled {
        router = router
            .merge(dashboard_auth_routes)
            .merge(dashboard_protected_routes);
    }

    router
        .layer(axum_mw::from_fn(
            middleware::request_logging::request_logging_middleware,
        ))
        .layer(axum_mw::from_fn(
            middleware::request_context::request_context_middleware,
        ))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
