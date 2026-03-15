use axum::Json;
use axum::extract::State;
use prism_domain::capability::{
    ProviderCapabilities, UpstreamProtocol, default_capabilities_for_protocol,
};
use prism_domain::operation::{ExecutionMode, IngressProtocol};
use serde::Serialize;

use crate::AppState;
use crate::handler::dashboard::providers::{ProbeStatus, cached_probe_result};

// ─── Protocol Matrix (#219) ────────────────────────────────────────────────

/// GET /api/dashboard/protocols/matrix
/// Returns which protocols, operations, and execution modes the gateway supports.
pub async fn protocol_matrix(State(state): State<AppState>) -> Json<ProtocolMatrixResponse> {
    let config = state.config.load();
    let mut entries = Vec::new();

    for provider in &config.providers {
        let up = prism_core::provider::upstream_protocol_for_kind(provider.upstream_kind());
        let caps = default_capabilities_for_protocol(up);
        let probe = cached_probe_result(&state, &provider.name);
        let stream_state = probe_state(&probe, "stream");
        let count_tokens_state = probe_state(&probe, "count_tokens");

        for protocol in [
            IngressProtocol::OpenAi,
            IngressProtocol::Claude,
            IngressProtocol::Gemini,
        ] {
            let exec_mode = up.execution_mode_for(protocol);
            entries.push(ProtocolMatrixEntry {
                provider: provider.name.clone(),
                ingress_protocol: protocol,
                upstream_protocol: up,
                execution_mode: exec_mode,
                supports_generate: !provider.disabled,
                supports_stream: caps.supports_stream
                    && stream_state.status == ProbeStatus::Verified,
                stream_state: stream_state.clone(),
                supports_count_tokens: caps.supports_count_tokens
                    && count_tokens_state.status == ProbeStatus::Verified,
                count_tokens_state: count_tokens_state.clone(),
            });
        }
    }

    Json(ProtocolMatrixResponse { entries })
}

/// GET /api/dashboard/providers/capabilities
/// Returns capability declarations for all providers and their models.
pub async fn provider_capabilities(
    State(state): State<AppState>,
) -> Json<ProviderCapabilitiesResponse> {
    let config = state.config.load();
    let mut providers = Vec::new();

    for provider in &config.providers {
        let up = prism_core::provider::upstream_protocol_for_kind(provider.upstream_kind());
        let caps = default_capabilities_for_protocol(up);
        let probe = cached_probe_result(&state, &provider.name);

        let model_ids: Vec<String> = if provider.models.is_empty() {
            vec!["*".to_string()]
        } else {
            provider.models.iter().map(|m| m.id.clone()).collect()
        };

        providers.push(ProviderCapabilityEntry {
            name: provider.name.clone(),
            upstream_protocol: up,
            models: model_ids,
            capabilities: caps,
            probe: CapabilityProbeStates {
                text: probe_state(&probe, "text"),
                stream: probe_state(&probe, "stream"),
                tools: probe_state(&probe, "tools"),
                images: probe_state(&probe, "images"),
                json_schema: probe_state(&probe, "json_schema"),
                reasoning: probe_state(&probe, "reasoning"),
                count_tokens: probe_state(&probe, "count_tokens"),
            },
            disabled: provider.disabled,
        });
    }

    Json(ProviderCapabilitiesResponse { providers })
}

// ─── API Types ──────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ProtocolMatrixResponse {
    pub entries: Vec<ProtocolMatrixEntry>,
}

#[derive(Debug, Serialize)]
pub struct ProtocolMatrixEntry {
    pub provider: String,
    pub ingress_protocol: IngressProtocol,
    pub upstream_protocol: UpstreamProtocol,
    pub execution_mode: ExecutionMode,
    pub supports_generate: bool,
    pub supports_stream: bool,
    pub stream_state: CapabilityProbeState,
    pub supports_count_tokens: bool,
    pub count_tokens_state: CapabilityProbeState,
}

#[derive(Debug, Serialize)]
pub struct ProviderCapabilitiesResponse {
    pub providers: Vec<ProviderCapabilityEntry>,
}

#[derive(Debug, Serialize)]
pub struct ProviderCapabilityEntry {
    pub name: String,
    pub upstream_protocol: UpstreamProtocol,
    pub models: Vec<String>,
    pub capabilities: ProviderCapabilities,
    pub probe: CapabilityProbeStates,
    pub disabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityProbeStates {
    pub text: CapabilityProbeState,
    pub stream: CapabilityProbeState,
    pub tools: CapabilityProbeState,
    pub images: CapabilityProbeState,
    pub json_schema: CapabilityProbeState,
    pub reasoning: CapabilityProbeState,
    pub count_tokens: CapabilityProbeState,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityProbeState {
    pub status: ProbeStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

fn probe_state(
    probe: &Option<crate::handler::dashboard::providers::ProviderProbeResult>,
    capability: &str,
) -> CapabilityProbeState {
    let check = probe.as_ref().and_then(|result| {
        result
            .checks
            .iter()
            .find(|check| check.capability == capability)
    });
    CapabilityProbeState {
        status: check
            .map(|value| value.status)
            .unwrap_or(ProbeStatus::Unknown),
        message: check.and_then(|value| value.message.clone()),
    }
}
