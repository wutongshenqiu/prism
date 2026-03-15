//! Application struct that encapsulates server assembly and serving logic.

use arc_swap::ArcSwap;
use prism_core::cache::{MokaCache, ResponseCacheBackend};
use prism_core::config::{Config, ConfigWatcher};
use prism_core::rate_limit::CompositeRateLimiter;
use prism_lifecycle::signal::SignalHandler;
use prism_lifecycle::{self, Lifecycle};
use prism_provider::catalog::ProviderCatalog;
use prism_provider::health::HealthManager;
use prism_provider::routing::CredentialRouter;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Configuration for running the server, decoupled from CLI parsing.
pub struct RunConfig {
    pub config_path: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub log_level: String,
    pub daemon: bool,
    pub pid_file: Option<String>,
    pub shutdown_timeout: Option<u64>,
}

pub struct Application {
    config: Arc<ArcSwap<Config>>,
    app_router: axum::Router,
    config_path: String,
    credential_router: Arc<CredentialRouter>,
    catalog: Arc<ProviderCatalog>,
    health_manager: Arc<HealthManager>,
    auth_runtime: Arc<crate::auth_runtime::AuthRuntimeManager>,
    rate_limiter: Arc<CompositeRateLimiter>,
    cost_calculator: Arc<prism_core::cost::CostCalculator>,
    http_client_pool: Arc<prism_core::proxy::HttpClientPool>,
    lifecycle: Box<dyn Lifecycle>,
    shutdown_timeout: u64,
    #[cfg(unix)]
    _pid_file: Option<prism_lifecycle::pid_file::PidFile>,
}

impl Application {
    /// Build the application from a `RunConfig`: load config, build executors,
    /// router, translators, metrics, and acquire PID file.
    ///
    /// `log_store` is created externally so it can be shared with the
    /// `GatewayLogLayer` (which must be registered before the application
    /// is built).
    pub fn build(
        args: &RunConfig,
        preloaded_config: Config,
        log_store: Arc<dyn prism_core::request_log::LogStore>,
    ) -> anyhow::Result<Self> {
        let mut config = preloaded_config;

        // CLI overrides
        if let Some(ref host) = args.host {
            config.host = host.clone();
        }
        if let Some(port) = args.port {
            config.port = port;
        }
        if let Some(ref pid_file) = args.pid_file {
            config.daemon.pid_file = pid_file.clone();
        }
        if let Some(timeout) = args.shutdown_timeout {
            config.daemon.shutdown_timeout = timeout;
        }

        let shutdown_timeout = config.daemon.shutdown_timeout;

        // Acquire PID file (unix only)
        #[cfg(unix)]
        let _pid_file = if args.daemon {
            Some(prism_lifecycle::pid_file::PidFile::acquire(
                &config.daemon.pid_file,
            )?)
        } else {
            None
        };

        // Build shared HTTP client pool and provider components
        let http_client_pool = Arc::new(prism_core::proxy::HttpClientPool::new());
        let executors =
            prism_provider::build_registry(config.proxy_url.clone(), http_client_pool.clone());
        let default_cred_strategy = config
            .routing
            .profiles
            .get(&config.routing.default_profile)
            .map(|p| p.credential_policy.strategy)
            .unwrap_or_default();
        let auth_runtime = Arc::new(crate::auth_runtime::AuthRuntimeManager::new());
        auth_runtime
            .initialize(&args.config_path, &config)
            .map_err(anyhow::Error::msg)?;
        let credential_router = Arc::new(CredentialRouter::new(default_cred_strategy));
        credential_router.set_oauth_states(auth_runtime.oauth_snapshot());
        credential_router.update_from_config(&config);

        // Build catalog and health manager (from same credential data as router)
        let catalog = Arc::new(ProviderCatalog::new());
        let health_manager = Arc::new(HealthManager::new(Default::default()));
        {
            let cred_map = credential_router.credential_map();
            catalog.update_from_credentials(&cred_map);
        }

        let translators = Arc::new(prism_translator::build_registry());
        let executors = Arc::new(executors);

        tracing::info!("Loaded {} provider entries", config.providers.len(),);

        let rate_limiter = Arc::new(CompositeRateLimiter::new(&config.rate_limit));
        let cost_calculator = Arc::new(prism_core::cost::CostCalculator::new(&config.model_prices));

        // Initialize thinking signature cache (if enabled)
        let thinking_cache = if config.thinking_cache.enabled {
            tracing::info!(
                "Thinking signature cache enabled (max_entries={}, ttl={}s)",
                config.thinking_cache.max_entries,
                config.thinking_cache.ttl_secs
            );
            Some(Arc::new(prism_core::thinking_cache::ThinkingCache::new(
                &config.thinking_cache,
            )))
        } else {
            None
        };

        // Initialize response cache (if enabled)
        let response_cache: Option<Arc<dyn ResponseCacheBackend>> = if config.cache.enabled {
            tracing::info!(
                "Response cache enabled (max_entries={}, ttl={}s)",
                config.cache.max_entries,
                config.cache.ttl_secs
            );
            Some(Arc::new(MokaCache::new(&config.cache)))
        } else {
            None
        };

        let config = Arc::new(ArcSwap::from_pointee(config));
        let metrics = Arc::new(prism_core::metrics::Metrics::new());

        // Build AppState and router
        let state = crate::AppState {
            config: config.clone(),
            router: credential_router.clone(),
            executors,
            translators,
            metrics,
            log_store,
            config_path: Arc::new(Mutex::new(args.config_path.clone())),
            rate_limiter: rate_limiter.clone(),
            cost_calculator: cost_calculator.clone(),
            response_cache,
            thinking_cache,
            http_client_pool: http_client_pool.clone(),
            start_time: Instant::now(),
            login_limiter: Arc::new(crate::handler::dashboard::auth::LoginRateLimiter::new()),
            catalog: catalog.clone(),
            health_manager: health_manager.clone(),
            auth_runtime: auth_runtime.clone(),
            oauth_sessions: Arc::new(dashmap::DashMap::new()),
            device_sessions: Arc::new(dashmap::DashMap::new()),
            provider_probe_cache: Arc::new(dashmap::DashMap::new()),
        };
        let app_router = crate::build_router(state);

        // Detect lifecycle
        let lc = prism_lifecycle::detect_lifecycle();

        Ok(Self {
            config,
            app_router,
            config_path: args.config_path.clone(),
            credential_router,
            catalog,
            health_manager,
            auth_runtime,
            rate_limiter,
            cost_calculator,
            http_client_pool,
            lifecycle: lc,
            shutdown_timeout,
            #[cfg(unix)]
            _pid_file,
        })
    }

    /// Start serving HTTP/HTTPS, handle signals, and drain gracefully.
    pub async fn serve(self) -> anyhow::Result<()> {
        let Self {
            config,
            app_router,
            config_path,
            credential_router,
            catalog,
            health_manager: _health_manager,
            auth_runtime,
            rate_limiter,
            cost_calculator,
            http_client_pool,
            lifecycle,
            shutdown_timeout,
            #[cfg(unix)]
            _pid_file,
        } = self;

        // Start config file watcher
        let watcher_router = credential_router.clone();
        let watcher_catalog = catalog.clone();
        let watcher_rate_limiter = rate_limiter.clone();
        let watcher_cost_calculator = cost_calculator.clone();
        let watcher_pool = http_client_pool.clone();
        let watcher_auth_runtime = auth_runtime.clone();
        let _watcher = ConfigWatcher::start(config_path.clone(), config.clone(), move |new_cfg| {
            if let Err(err) = watcher_auth_runtime.sync_with_config(new_cfg) {
                tracing::error!("Auth runtime sync failed on config reload: {err}");
            }
            watcher_router.set_oauth_states(watcher_auth_runtime.oauth_snapshot());
            watcher_router.update_from_config(new_cfg);
            watcher_catalog.update_from_credentials(&watcher_router.credential_map());
            watcher_rate_limiter.update_config(&new_cfg.rate_limit);
            watcher_cost_calculator.update_prices(&new_cfg.model_prices);
            watcher_pool.clear();
            tracing::info!(
                "Config reloaded: {} provider entries",
                new_cfg.providers.len(),
            );
        });

        // Setup signal handler
        let (signal_handler, shutdown_rx) = SignalHandler::new();

        // SIGHUP reload function
        let reload_config = config.clone();
        let reload_router = credential_router.clone();
        let reload_catalog = catalog.clone();
        let reload_rate_limiter = rate_limiter.clone();
        let reload_cost_calculator = cost_calculator.clone();
        let reload_pool = http_client_pool;
        let reload_path = config_path.clone();
        let reload_auth_runtime = auth_runtime.clone();
        let reload_lifecycle: Arc<dyn Lifecycle> = Arc::from(prism_lifecycle::detect_lifecycle());
        let reload_fn = move || {
            reload_lifecycle.on_reloading();
            match Config::load(&reload_path) {
                Ok(new_cfg) => {
                    if let Err(err) = reload_auth_runtime.sync_with_config(&new_cfg) {
                        tracing::error!("Auth runtime sync failed on SIGHUP reload: {err}");
                    }
                    reload_router.set_oauth_states(reload_auth_runtime.oauth_snapshot());
                    reload_router.update_from_config(&new_cfg);
                    reload_catalog.update_from_credentials(&reload_router.credential_map());
                    reload_rate_limiter.update_config(&new_cfg.rate_limit);
                    reload_cost_calculator.update_prices(&new_cfg.model_prices);
                    reload_pool.clear();
                    tracing::info!(
                        "SIGHUP reload: {} provider entries",
                        new_cfg.providers.len(),
                    );
                    reload_config.store(Arc::new(new_cfg));
                    reload_lifecycle.on_reloaded();
                }
                Err(e) => {
                    tracing::error!("SIGHUP config reload failed: {e}");
                }
            }
        };

        // Spawn signal handler
        tokio::spawn(signal_handler.run(reload_fn));

        // Bind and serve
        let cfg = config.load();
        let addr = format!("{}:{}", cfg.host, cfg.port);

        if cfg.tls.enable {
            serve_tls(
                &addr,
                &cfg,
                app_router,
                shutdown_rx,
                &*lifecycle,
                shutdown_timeout,
            )
            .await?;
        } else {
            serve_http(
                &addr,
                app_router,
                shutdown_rx,
                &*lifecycle,
                shutdown_timeout,
            )
            .await?;
        }

        tracing::info!("Server shut down.");
        Ok(())
    }
}

/// Top-level entry point: daemonize, init logging, build & serve.
pub fn run(args: RunConfig) -> anyhow::Result<()> {
    // Daemonize before creating tokio runtime (unix only)
    #[cfg(unix)]
    if args.daemon {
        prism_lifecycle::daemon::daemonize()?;
    }

    // Load config once — fail fast if invalid (never fall back to defaults)
    let config = Config::load(&args.config_path)?;

    // Init logging — force file logging when running as daemon
    let to_file = args.daemon || config.logging_to_file;
    let log_dir = config.log_dir.clone();

    // Create log store before logging init so it can be shared with both
    // the GatewayLogLayer and the Application.
    let file_writer = if config.log_store.file_audit.enabled {
        match prism_core::file_audit::FileAuditWriter::new(&config.log_store.file_audit) {
            Ok(w) => Some(w),
            Err(e) => {
                eprintln!("Failed to initialize file audit writer: {e}, file audit disabled");
                None
            }
        }
    } else {
        None
    };
    let log_store: Arc<dyn prism_core::request_log::LogStore> = Arc::new(
        prism_core::memory_log_store::InMemoryLogStore::new(config.log_store.capacity, file_writer),
    );

    let gateway_layer = crate::telemetry::GatewayLogLayer::new(log_store.clone());

    let _guard = prism_lifecycle::logging::init_logging_with_layer(
        &args.log_level,
        to_file,
        log_dir.as_deref(),
        Box::new(gateway_layer),
    );

    // Build and run on a multi-thread runtime
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async {
        // Spawn file audit cleanup task inside the tokio runtime
        if config.log_store.file_audit.enabled {
            prism_core::file_audit::FileAuditWriter::spawn_cleanup_static(
                config.log_store.file_audit.dir.clone(),
                config.log_store.file_audit.retention_days,
            );
        }
        let application = Application::build(&args, config, log_store)?;
        application.serve().await
    })
}

async fn serve_http(
    addr: &str,
    app_router: axum::Router,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
    lifecycle: &dyn Lifecycle,
    shutdown_timeout: u64,
) -> anyhow::Result<()> {
    tracing::info!("Starting HTTP server on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    lifecycle.on_ready();

    let shutdown = async move {
        let _ = shutdown_rx.wait_for(|v| *v).await;
    };

    axum::serve(
        listener,
        app_router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .await?;

    lifecycle.on_stopping();
    tokio::time::sleep(Duration::from_secs(shutdown_timeout.min(1))).await;
    Ok(())
}

async fn serve_tls(
    addr: &str,
    cfg: &Config,
    app_router: axum::Router,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
    lifecycle: &dyn Lifecycle,
    shutdown_timeout: u64,
) -> anyhow::Result<()> {
    let cert_path = cfg.tls.cert.as_ref().expect("TLS cert required");
    let key_path = cfg.tls.key.as_ref().expect("TLS key required");

    use rustls_pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};

    let certs: Vec<CertificateDer<'static>> =
        CertificateDer::pem_file_iter(cert_path)?.collect::<Result<Vec<_>, _>>()?;
    let key = PrivateKeyDer::from_pem_file(key_path)?;

    let tls_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;
    let tls_acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(tls_config));

    tracing::info!("Starting HTTPS server on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    lifecycle.on_ready();

    loop {
        tokio::select! {
            result = listener.accept() => {
                let (stream, peer_addr) = result?;
                let acceptor = tls_acceptor.clone();
                let router = app_router.clone();
                tokio::spawn(async move {
                    match acceptor.accept(stream).await {
                        Ok(tls_stream) => {
                            let io = hyper_util::rt::TokioIo::new(tls_stream);
                            let service = hyper::service::service_fn(
                                move |req: hyper::Request<hyper::body::Incoming>| {
                                    let router = router.clone();
                                    async move {
                                        let (mut parts, body) = req.into_parts();
                                        let body = axum::body::Body::new(body);
                                        // Inject ConnectInfo so middleware can read the peer address
                                        parts.extensions.insert(axum::extract::ConnectInfo(peer_addr));
                                        let req = axum::http::Request::from_parts(parts, body);
                                        Ok::<_, std::convert::Infallible>(
                                            tower::ServiceExt::oneshot(router, req)
                                                .await
                                                .expect("infallible"),
                                        )
                                    }
                                },
                            );
                            if let Err(e) = hyper_util::server::conn::auto::Builder::new(
                                hyper_util::rt::TokioExecutor::new(),
                            )
                            .serve_connection(io, service)
                            .await
                            {
                                tracing::error!("TLS connection error from {peer_addr}: {e}");
                            }
                        }
                        Err(e) => tracing::error!("TLS accept error from {peer_addr}: {e}"),
                    }
                });
            }
            _ = shutdown_rx.wait_for(|v| *v) => {
                tracing::info!("Stopping TLS listener, waiting for connections to drain...");
                break;
            }
        }
    }

    lifecycle.on_stopping();
    tokio::time::sleep(Duration::from_secs(shutdown_timeout.min(5))).await;
    Ok(())
}
