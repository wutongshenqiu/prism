use axum::Json;
use axum::extract::State;
use prism_domain::capability::{
    ProviderCapabilities, UpstreamProtocol, default_capabilities_for_protocol,
};
use prism_domain::operation::{ExecutionMode, IngressProtocol};
use serde::Serialize;

use crate::AppState;

// ─── Protocol Matrix (#219) ────────────────────────────────────────────────

/// GET /api/dashboard/protocols/matrix
/// Returns which protocols, operations, and execution modes the gateway supports.
pub async fn protocol_matrix(State(state): State<AppState>) -> Json<ProtocolMatrixResponse> {
    let config = state.config.load();
    let mut entries = Vec::new();

    for provider in &config.providers {
        let up = prism_core::provider::upstream_protocol(provider.format);
        let caps = default_capabilities_for_protocol(up);

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
                supports_stream: caps.supports_stream,
                supports_count_tokens: caps.supports_count_tokens,
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
        let up = prism_core::provider::upstream_protocol(provider.format);
        let caps = default_capabilities_for_protocol(up);

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
    pub supports_count_tokens: bool,
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
    pub disabled: bool,
}
