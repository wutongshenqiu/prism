//! Application lifecycle management: readiness notification, signal handling,
//! daemonization, PID file management, and logging.

#[cfg(unix)]
pub mod daemon;
pub mod logging;
pub mod notify;
#[cfg(unix)]
pub mod pid_file;
pub mod signal;

/// Trait for lifecycle event notification (foreground vs systemd).
pub trait Lifecycle: Send + Sync {
    /// Called when the server is ready to accept connections.
    fn on_ready(&self);
    /// Called when configuration reload begins.
    fn on_reloading(&self);
    /// Called when configuration reload completes.
    fn on_reloaded(&self);
    /// Called when the server is about to stop.
    fn on_stopping(&self);
}

/// Foreground lifecycle — logs events only.
pub struct ForegroundLifecycle;

impl Lifecycle for ForegroundLifecycle {
    fn on_ready(&self) {
        tracing::info!("Service ready");
    }

    fn on_reloading(&self) {
        tracing::info!("Service reloading configuration...");
    }

    fn on_reloaded(&self) {
        tracing::info!("Service configuration reloaded");
    }

    fn on_stopping(&self) {
        tracing::info!("Service stopping...");
    }
}

/// Systemd lifecycle — sends sd-notify messages and logs.
pub struct SystemdLifecycle;

impl Lifecycle for SystemdLifecycle {
    fn on_ready(&self) {
        notify::sd_ready();
        tracing::info!("Service ready (notified systemd)");
    }

    fn on_reloading(&self) {
        notify::sd_reloading();
        tracing::info!("Service reloading configuration (notified systemd)...");
    }

    fn on_reloaded(&self) {
        notify::sd_ready();
        tracing::info!("Service configuration reloaded (notified systemd)");
    }

    fn on_stopping(&self) {
        notify::sd_stopping();
        tracing::info!("Service stopping (notified systemd)...");
    }
}

/// Auto-detect the appropriate lifecycle implementation based on environment.
/// Returns `SystemdLifecycle` if `NOTIFY_SOCKET` is set, else `ForegroundLifecycle`.
pub fn detect_lifecycle() -> Box<dyn Lifecycle> {
    if std::env::var("NOTIFY_SOCKET").is_ok() {
        Box::new(SystemdLifecycle)
    } else {
        Box::new(ForegroundLifecycle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_foreground_lifecycle_no_panic() {
        let lc = ForegroundLifecycle;
        lc.on_ready();
        lc.on_reloading();
        lc.on_reloaded();
        lc.on_stopping();
    }

    #[test]
    fn test_systemd_lifecycle_no_panic() {
        // sd-notify calls silently fail when NOTIFY_SOCKET is not set
        let lc = SystemdLifecycle;
        lc.on_ready();
        lc.on_reloading();
        lc.on_reloaded();
        lc.on_stopping();
    }

    #[test]
    fn test_detect_lifecycle_foreground() {
        // Without NOTIFY_SOCKET, should get ForegroundLifecycle
        // SAFETY: This test doesn't run concurrently with other tests that
        // read NOTIFY_SOCKET.
        unsafe {
            std::env::remove_var("NOTIFY_SOCKET");
        }
        let _lc = detect_lifecycle();
        // Just ensure it doesn't panic
    }
}
