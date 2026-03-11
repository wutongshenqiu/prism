//! Logging initialization with optional file-based daily rotation.

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Initialize the tracing subscriber.
///
/// - `to_file=true` → daily rotating file appender with non-blocking writer
/// - `to_file=false` → stderr output (default)
///
/// Returns an `Option<WorkerGuard>` that **must be held** for the lifetime of
/// the application to ensure buffered logs are flushed on shutdown.
pub fn init_logging(level: &str, to_file: bool, log_dir: Option<&str>) -> Option<WorkerGuard> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));

    if to_file {
        let dir = log_dir.unwrap_or("./logs");
        let file_appender = tracing_appender::rolling::daily(dir, "prism.log");
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

        tracing_subscriber::registry()
            .with(env_filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .with_writer(non_blocking)
                    .with_ansi(false),
            )
            .init();

        Some(guard)
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer())
            .init();

        None
    }
}

/// Initialize tracing with an extra layer (e.g., GatewayLogLayer).
///
/// The extra layer is added to the registry alongside the fmt layer.
/// Uses a boxed layer to avoid complex generic type constraints.
pub fn init_logging_with_layer(
    level: &str,
    to_file: bool,
    log_dir: Option<&str>,
    extra_layer: Box<dyn tracing_subscriber::Layer<tracing_subscriber::Registry> + Send + Sync>,
) -> Option<WorkerGuard> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));

    // Add the extra layer first (directly on Registry), then fmt + filter on top.
    if to_file {
        let dir = log_dir.unwrap_or("./logs");
        let file_appender = tracing_appender::rolling::daily(dir, "prism.log");
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

        tracing_subscriber::registry()
            .with(extra_layer)
            .with(env_filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .with_writer(non_blocking)
                    .with_ansi(false),
            )
            .init();

        Some(guard)
    } else {
        tracing_subscriber::registry()
            .with(extra_layer)
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer())
            .init();

        None
    }
}
