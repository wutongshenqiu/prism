use crate::request::{Rejection, RequiredCapabilities};

/// Errors from the canonical runtime pipeline.
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("no provider can serve this request: {reason}")]
    NoCapableProvider {
        reason: String,
        rejections: Vec<Rejection>,
        required: RequiredCapabilities,
    },

    #[error("unsupported operation: {0}")]
    UnsupportedOperation(String),

    #[error("ingress adapter error: {0}")]
    IngressError(String),

    #[error("egress adapter error: {0}")]
    EgressError(String),

    #[error("provider execution error: {0}")]
    ExecutionError(String),

    #[error("model not found: {0}")]
    ModelNotFound(String),

    #[error("{0}")]
    Internal(String),
}
