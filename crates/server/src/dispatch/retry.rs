use crate::AppState;
use prism_core::config::RetryConfig;
use prism_core::error::ProxyError;

/// Handle retry-related side effects (circuit breaker, logging).
pub(super) fn handle_retry_error(
    state: &AppState,
    auth_id: &str,
    error: &ProxyError,
    _retry_cfg: &RetryConfig,
) {
    state.metrics.record_error();
    match error {
        ProxyError::Upstream { status, .. } => match *status {
            429 => {
                state.router.record_failure(auth_id);
                tracing::warn!("Rate limited (429), recording circuit breaker failure");
            }
            s if (500..=599).contains(&s) => {
                state.router.record_failure(auth_id);
                tracing::warn!("Upstream error ({s}), recording circuit breaker failure");
            }
            _ => {}
        },
        ProxyError::Network(_) => {
            state.router.record_failure(auth_id);
            tracing::warn!("Network error, recording circuit breaker failure");
        }
        _ => {}
    }
}
