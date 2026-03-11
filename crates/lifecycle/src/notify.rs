//! Thin wrappers around sd-notify for systemd readiness protocol.

/// Notify systemd that the service is ready.
pub fn sd_ready() {
    let _ = sd_notify::notify(true, &[sd_notify::NotifyState::Ready]);
}

/// Notify systemd that the service is reloading configuration.
pub fn sd_reloading() {
    let _ = sd_notify::notify(true, &[sd_notify::NotifyState::Reloading]);
}

/// Notify systemd that the service is stopping.
pub fn sd_stopping() {
    let _ = sd_notify::notify(true, &[sd_notify::NotifyState::Stopping]);
}
