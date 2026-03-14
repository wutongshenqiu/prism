use axum::Json;
use axum::extract::State;
use prism_domain::capability::{
    ProviderCapabilities, UpstreamProtocol, default_capabilities_for_protocol,
};
use prism_domain::operation::{ExecutionMode, IngressProtocol, Operation};
use prism_domain::request::{
    ExplainFeatures, ExplainRequest, ExplainResponse, Rejection, RejectionReason, SelectedRoute,
};
use serde::{Deserialize, Serialize};

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
                supports_generate: true,
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

// ─── Route Explain & Replay (#220) ─────────────────────────────────────────

/// POST /api/dashboard/routing/explain
/// Explain routing decisions without executing.
pub async fn route_explain(
    State(state): State<AppState>,
    Json(req): Json<ExplainApiRequest>,
) -> Json<ExplainResponse> {
    let ingress = match req.ingress_protocol.as_str() {
        "claude" => IngressProtocol::Claude,
        "gemini" => IngressProtocol::Gemini,
        _ => IngressProtocol::OpenAi,
    };

    let operation = match req.operation.as_str() {
        "count_tokens" => Operation::CountTokens,
        "list_models" => Operation::ListModels,
        _ => Operation::Generate,
    };

    let endpoint = match ingress {
        IngressProtocol::OpenAi => prism_domain::operation::Endpoint::ChatCompletions,
        IngressProtocol::Claude => prism_domain::operation::Endpoint::Messages,
        IngressProtocol::Gemini => prism_domain::operation::Endpoint::GenerateContent,
    };

    let explain_req = ExplainRequest {
        ingress_protocol: ingress,
        operation,
        endpoint,
        model: req.model.clone(),
        stream: req.stream,
        features: req.features.clone(),
        tenant_id: req.tenant_id.clone(),
        api_key_id: req.api_key_id.clone(),
        region: req.region.clone(),
    };

    let required = explain_req.required_capabilities();

    // Build inventory snapshot and run planner
    let inventory = state.catalog.snapshot();
    let health = state.health_manager.snapshot();
    let config = state.config.load();

    let features = prism_core::routing::types::RouteRequestFeatures {
        requested_model: req.model.clone(),
        endpoint: match ingress {
            IngressProtocol::OpenAi => prism_core::routing::types::RouteEndpoint::ChatCompletions,
            IngressProtocol::Claude => prism_core::routing::types::RouteEndpoint::Messages,
            IngressProtocol::Gemini => prism_core::routing::types::RouteEndpoint::GenerateContent,
        },
        source_format: match ingress {
            IngressProtocol::OpenAi => prism_core::provider::Format::OpenAI,
            IngressProtocol::Claude => prism_core::provider::Format::Claude,
            IngressProtocol::Gemini => prism_core::provider::Format::Gemini,
        },
        tenant_id: req.tenant_id,
        api_key_id: req.api_key_id,
        region: req.region,
        stream: req.stream,
        headers: Default::default(),
        required_capabilities: Some(required.clone()),
    };

    let plan = prism_core::routing::planner::RoutePlanner::plan(
        &features,
        &config.routing,
        &inventory,
        &health,
    );

    // Convert plan to ExplainResponse
    let to_selected = |a: &prism_core::routing::types::RouteAttemptPlan| SelectedRoute {
        provider: a.credential_name.clone(),
        credential: a.credential_id.clone(),
        model: a.model.clone(),
        execution_mode: a.execution_mode.unwrap_or(ExecutionMode::LosslessAdapted),
    };

    let selected = plan.attempts.first().map(&to_selected);

    let alternates: Vec<SelectedRoute> = plan.attempts.iter().skip(1).map(to_selected).collect();

    let rejections: Vec<Rejection> = plan
        .trace
        .rejections
        .iter()
        .map(|r| {
            let reason = match &r.reason {
                prism_core::routing::types::RejectReason::ModelNotSupported => {
                    RejectionReason::ModelNotSupported {
                        model: req.model.clone(),
                    }
                }
                prism_core::routing::types::RejectReason::CircuitBreakerOpen => {
                    RejectionReason::CircuitOpen
                }
                prism_core::routing::types::RejectReason::CredentialDisabled => {
                    RejectionReason::Disabled
                }
                prism_core::routing::types::RejectReason::MissingCapability { capabilities } => {
                    RejectionReason::MissingCapability {
                        capability: capabilities.join(", "),
                    }
                }
                _ => RejectionReason::CredentialUnavailable,
            };
            Rejection {
                candidate: r.candidate.clone(),
                reason,
            }
        })
        .collect();

    Json(ExplainResponse {
        selected,
        alternates,
        rejections,
        required_capabilities: required,
    })
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

#[derive(Debug, Deserialize)]
pub struct ExplainApiRequest {
    #[serde(default = "default_openai")]
    pub ingress_protocol: String,
    #[serde(default = "default_generate")]
    pub operation: String,
    pub model: String,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub features: ExplainFeatures,
    #[serde(default)]
    pub tenant_id: Option<String>,
    #[serde(default)]
    pub api_key_id: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
}

fn default_openai() -> String {
    "openai".to_string()
}
fn default_generate() -> String {
    "generate".to_string()
}
