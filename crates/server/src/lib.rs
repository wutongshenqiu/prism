pub mod auth;
pub mod dispatch;
pub mod handler;
pub mod middleware;
pub mod streaming;

use ai_proxy_core::config::Config;
use ai_proxy_core::metrics::Metrics;
use ai_proxy_core::request_log::RequestLogStore;
use ai_proxy_provider::ExecutorRegistry;
use ai_proxy_provider::routing::CredentialRouter;
use ai_proxy_translator::TranslatorRegistry;
use arc_swap::ArcSwap;
use axum::{Router, middleware as axum_mw};
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
    pub request_logs: Arc<RequestLogStore>,
    pub config_path: Arc<Mutex<String>>,
    pub credential_router: Arc<CredentialRouter>,
    pub start_time: Instant,
}

pub fn build_router(state: AppState) -> Router {
    let body_limit_bytes = state.config.load().body_limit_mb * 1024 * 1024;

    // Public routes — no auth required
    let public_routes = Router::new()
        .route("/health", axum::routing::get(handler::health::health))
        .route("/metrics", axum::routing::get(handler::health::metrics));

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
            "/v1/responses",
            axum::routing::post(handler::responses::responses),
        )
        .layer(RequestBodyLimitLayer::new(body_limit_bytes))
        .layer(axum_mw::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ));

    // Dashboard auth routes — no auth required (login endpoint)
    let dashboard_auth_routes = Router::new().route(
        "/api/dashboard/auth/login",
        axum::routing::post(handler::dashboard::auth::login),
    );

    // Dashboard protected routes — JWT auth required
    let dashboard_protected_routes = Router::new()
        .route(
            "/api/dashboard/auth/refresh",
            axum::routing::post(handler::dashboard::auth::refresh),
        )
        // Providers
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
            axum::routing::delete(handler::dashboard::auth_keys::delete_auth_key),
        )
        // Routing
        .route(
            "/api/dashboard/routing",
            axum::routing::get(handler::dashboard::routing::get_routing)
                .patch(handler::dashboard::routing::update_routing),
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
            "/api/dashboard/config/current",
            axum::routing::get(handler::dashboard::config_ops::get_config),
        )
        // Request logs
        .route(
            "/api/dashboard/logs",
            axum::routing::get(handler::dashboard::logs::query_logs),
        )
        .route(
            "/api/dashboard/logs/stats",
            axum::routing::get(handler::dashboard::logs::log_stats),
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
        .layer(axum_mw::from_fn_with_state(
            state.clone(),
            middleware::dashboard_auth::dashboard_auth_middleware,
        ));

    // WebSocket routes (auth via query param)
    let ws_routes = Router::new().route(
        "/ws/dashboard",
        axum::routing::get(handler::dashboard::websocket::ws_handler),
    );

    // Compose: public + admin + api + dashboard, then global middleware layers (outer → inner)
    Router::new()
        .merge(public_routes)
        .merge(admin_routes)
        .merge(api_routes)
        .merge(dashboard_auth_routes)
        .merge(dashboard_protected_routes)
        .merge(ws_routes)
        .layer(axum_mw::from_fn_with_state(
            state.clone(),
            middleware::request_logging::request_logging_middleware,
        ))
        .layer(axum_mw::from_fn(
            middleware::request_context::request_context_middleware,
        ))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
