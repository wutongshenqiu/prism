//! Daemonize the process. Must be called BEFORE the tokio runtime is created.

/// Fork the process into a daemon. The parent exits, the child continues
/// in the background. `keep_cwd=true` preserves the working directory;
/// stdio is redirected to `/dev/null`.
///
/// # Errors
/// Returns an error if the fork fails.
pub fn daemonize() -> anyhow::Result<()> {
    match fork::daemon(true, false) {
        Ok(fork::Fork::Parent(_)) => std::process::exit(0),
        Ok(fork::Fork::Child) => {
            tracing::info!("Daemonized successfully, PID {}", std::process::id());
            Ok(())
        }
        Err(e) => anyhow::bail!("Failed to daemonize: {}", e),
    }
}
