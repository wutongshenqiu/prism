//! Unified signal handling for shutdown (SIGTERM/SIGINT) and reload (SIGHUP).

use tokio::sync::watch;

/// A signal handler that listens for OS signals and dispatches shutdown/reload.
pub struct SignalHandler {
    shutdown_tx: watch::Sender<bool>,
}

impl SignalHandler {
    /// Create a new signal handler and a receiver that becomes `true` on shutdown.
    pub fn new() -> (Self, watch::Receiver<bool>) {
        let (tx, rx) = watch::channel(false);
        (Self { shutdown_tx: tx }, rx)
    }

    /// Run the signal loop. Blocks until a shutdown signal is received.
    ///
    /// - SIGTERM / SIGINT / Ctrl+C → triggers shutdown
    /// - SIGHUP (unix only) → calls `reload_fn`
    pub async fn run<F>(self, reload_fn: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{SignalKind, signal};

            let mut sigterm =
                signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
            let mut sighup =
                signal(SignalKind::hangup()).expect("failed to install SIGHUP handler");

            loop {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {
                        tracing::info!("Received SIGINT, initiating shutdown...");
                        break;
                    }
                    _ = sigterm.recv() => {
                        tracing::info!("Received SIGTERM, initiating shutdown...");
                        break;
                    }
                    _ = sighup.recv() => {
                        tracing::info!("Received SIGHUP, reloading configuration...");
                        reload_fn();
                    }
                }
            }
        }

        #[cfg(not(unix))]
        {
            let _ = &reload_fn; // suppress unused warning
            tokio::signal::ctrl_c()
                .await
                .expect("failed to install Ctrl+C handler");
            tracing::info!("Received Ctrl+C, initiating shutdown...");
        }

        let _ = self.shutdown_tx.send(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_handler_construction() {
        let (handler, rx) = SignalHandler::new();
        assert!(!*rx.borrow());
        // Sending shutdown manually
        let _ = handler.shutdown_tx.send(true);
        assert!(*rx.borrow());
    }
}
