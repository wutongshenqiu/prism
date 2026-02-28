//! Application struct that encapsulates server assembly and serving logic.

use crate::cli::RunArgs;
use ai_proxy_core::config::{Config, ConfigWatcher};
use ai_proxy_core::lifecycle::signal::SignalHandler;
use ai_proxy_core::lifecycle::{self, Lifecycle};
use ai_proxy_provider::routing::CredentialRouter;
use arc_swap::ArcSwap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct Application {
    config: Arc<ArcSwap<Config>>,
    app_router: axum::Router,
    config_path: String,
    credential_router: Arc<CredentialRouter>,
    lifecycle: Box<dyn Lifecycle>,
    shutdown_timeout: u64,
    #[cfg(unix)]
    _pid_file: Option<ai_proxy_core::lifecycle::pid_file::PidFile>,
}

impl Application {
    /// Build the application from CLI args: load config, build executors,
    /// router, translators, metrics, and acquire PID file.
    pub fn build(args: &RunArgs) -> anyhow::Result<Self> {
        // Load config
        let mut config = Config::load(&args.config).unwrap_or_else(|e| {
            tracing::warn!(
                "Failed to load config from '{}': {e}, using defaults",
                args.config
            );
            Config::default()
        });

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
            Some(ai_proxy_core::lifecycle::pid_file::PidFile::acquire(
                &config.daemon.pid_file,
            )?)
        } else {
            None
        };

        // Build provider components
        let executors = ai_proxy_provider::build_registry(config.proxy_url.clone());
        let credential_router = Arc::new(CredentialRouter::new(config.routing.strategy.clone()));
        credential_router.update_from_config(&config);
        let translators = Arc::new(ai_proxy_translator::build_registry());
        let executors = Arc::new(executors);

        tracing::info!(
            "Loaded {} claude keys, {} openai keys, {} gemini keys, {} compat keys",
            config.claude_api_key.len(),
            config.openai_api_key.len(),
            config.gemini_api_key.len(),
            config.openai_compatibility.len(),
        );

        let request_log_capacity = config.dashboard.request_log_capacity;
        let config = Arc::new(ArcSwap::from_pointee(config));
        let metrics = Arc::new(ai_proxy_core::metrics::Metrics::new());
        let request_logs = Arc::new(ai_proxy_core::request_log::RequestLogStore::new(
            request_log_capacity,
        ));

        // Build AppState and router
        let state = ai_proxy_server::AppState {
            config: config.clone(),
            router: credential_router.clone(),
            executors,
            translators,
            metrics,
            request_logs,
            config_path: Arc::new(Mutex::new(args.config.clone())),
            credential_router: credential_router.clone(),
            start_time: Instant::now(),
        };
        let app_router = ai_proxy_server::build_router(state);

        // Detect lifecycle
        let lc = lifecycle::detect_lifecycle();

        Ok(Self {
            config,
            app_router,
            config_path: args.config.clone(),
            credential_router,
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
            lifecycle,
            shutdown_timeout,
            #[cfg(unix)]
            _pid_file,
        } = self;

        // Start config file watcher
        let watcher_router = credential_router.clone();
        let _watcher = ConfigWatcher::start(config_path.clone(), config.clone(), move |new_cfg| {
            watcher_router.update_from_config(new_cfg);
            tracing::info!(
                "Config reloaded: {} claude keys, {} openai keys, {} gemini keys, {} compat keys",
                new_cfg.claude_api_key.len(),
                new_cfg.openai_api_key.len(),
                new_cfg.gemini_api_key.len(),
                new_cfg.openai_compatibility.len(),
            );
        });

        // Setup signal handler
        let (signal_handler, shutdown_rx) = SignalHandler::new();

        // SIGHUP reload function
        let reload_config = config.clone();
        let reload_router = credential_router.clone();
        let reload_path = config_path.clone();
        let reload_lifecycle: Arc<dyn Lifecycle> = Arc::from(lifecycle::detect_lifecycle());
        let reload_fn = move || {
            reload_lifecycle.on_reloading();
            match Config::load(&reload_path) {
                Ok(new_cfg) => {
                    reload_router.update_from_config(&new_cfg);
                    tracing::info!(
                        "SIGHUP reload: {} claude keys, {} openai keys, {} gemini keys, {} compat keys",
                        new_cfg.claude_api_key.len(),
                        new_cfg.openai_api_key.len(),
                        new_cfg.gemini_api_key.len(),
                        new_cfg.openai_compatibility.len(),
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

    axum::serve(listener, app_router)
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
                                        let (parts, body) = req.into_parts();
                                        let body = axum::body::Body::new(body);
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
