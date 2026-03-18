mod auth_profile_state;
mod helpers;
mod mutation;
mod probe;
mod read;

use serde::{Deserialize, Serialize};

pub use mutation::{create_provider, delete_provider, update_provider};
pub use probe::{
    cached_probe_result, fetch_models, health_check, presentation_preview, test_request,
};
pub use read::{get_provider, list_providers};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProbeStatus {
    Verified,
    Failed,
    Unknown,
    Unsupported,
}

impl ProbeStatus {
    fn is_verified(self) -> bool {
        matches!(self, Self::Verified)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderProbeCheck {
    pub capability: String,
    pub status: ProbeStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderProbeResult {
    pub provider: String,
    pub upstream: String,
    pub status: String,
    pub checked_at: String,
    pub latency_ms: u64,
    pub checks: Vec<ProviderProbeCheck>,
}

impl ProviderProbeResult {
    pub fn capability_status(&self, capability: &str) -> ProbeStatus {
        self.checks
            .iter()
            .find(|check| check.capability == capability)
            .map(|check| check.status)
            .unwrap_or(ProbeStatus::Unknown)
    }
}
