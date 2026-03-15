use axum::Json;
use axum::extract::State;
use prism_core::config::ProviderKeyEntry;
use prism_core::presentation::{ActivationMode, ProfileKind};
use prism_core::provider::{Format, UpstreamKind, WireApi, upstream_protocol_for_kind};
use prism_domain::capability::{
    ProviderCapabilities, UpstreamProtocol, default_capabilities_for_protocol,
};
use prism_domain::operation::{ExecutionMode, IngressProtocol, Operation};
use serde::Serialize;

use crate::AppState;
use crate::handler::dashboard::providers::{ProbeStatus, ProviderProbeResult, cached_probe_result};

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EndpointScope {
    Public,
    ProviderScoped,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EndpointTransport {
    Http,
    WebSocket,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamTransport {
    None,
    Sse,
    WebSocketEvents,
}

#[derive(Debug, Clone, Copy)]
enum SurfaceProbeKind {
    Text,
    Stream,
    CountTokens,
}

#[derive(Debug, Clone, Copy)]
struct SurfaceSpec {
    id: &'static str,
    label: &'static str,
    ingress_protocol: IngressProtocol,
    allowed_formats: &'static [Format],
    probe_kind: SurfaceProbeKind,
}

#[derive(Debug, Clone, Copy)]
struct EndpointSpec {
    id: &'static str,
    family: IngressProtocol,
    method: &'static str,
    path: &'static str,
    description: &'static str,
    scope: EndpointScope,
    transport: EndpointTransport,
    operation: Operation,
    stream_transport: StreamTransport,
    surface_id: Option<&'static str>,
    note: Option<&'static str>,
}

const ANY_FORMAT: &[Format] = &[Format::OpenAI, Format::Claude, Format::Gemini];
const OPENAI_ONLY: &[Format] = &[Format::OpenAI];
const CLAUDE_ONLY: &[Format] = &[Format::Claude];

const SURFACE_SPECS: &[SurfaceSpec] = &[
    SurfaceSpec {
        id: "openai_chat",
        label: "OpenAI Chat",
        ingress_protocol: IngressProtocol::OpenAi,
        allowed_formats: ANY_FORMAT,
        probe_kind: SurfaceProbeKind::Text,
    },
    SurfaceSpec {
        id: "openai_responses",
        label: "OpenAI Responses",
        ingress_protocol: IngressProtocol::OpenAi,
        allowed_formats: OPENAI_ONLY,
        probe_kind: SurfaceProbeKind::Text,
    },
    SurfaceSpec {
        id: "openai_responses_ws",
        label: "OpenAI Responses WS",
        ingress_protocol: IngressProtocol::OpenAi,
        allowed_formats: OPENAI_ONLY,
        probe_kind: SurfaceProbeKind::Stream,
    },
    SurfaceSpec {
        id: "claude_messages",
        label: "Claude Messages",
        ingress_protocol: IngressProtocol::Claude,
        allowed_formats: ANY_FORMAT,
        probe_kind: SurfaceProbeKind::Text,
    },
    SurfaceSpec {
        id: "claude_count_tokens",
        label: "Claude Count Tokens",
        ingress_protocol: IngressProtocol::Claude,
        allowed_formats: CLAUDE_ONLY,
        probe_kind: SurfaceProbeKind::CountTokens,
    },
    SurfaceSpec {
        id: "gemini_generate",
        label: "Gemini Generate",
        ingress_protocol: IngressProtocol::Gemini,
        allowed_formats: ANY_FORMAT,
        probe_kind: SurfaceProbeKind::Text,
    },
    SurfaceSpec {
        id: "gemini_stream",
        label: "Gemini Stream",
        ingress_protocol: IngressProtocol::Gemini,
        allowed_formats: ANY_FORMAT,
        probe_kind: SurfaceProbeKind::Stream,
    },
];

const ENDPOINT_SPECS: &[EndpointSpec] = &[
    EndpointSpec {
        id: "openai_chat_completions",
        family: IngressProtocol::OpenAi,
        method: "POST",
        path: "/v1/chat/completions",
        description: "Unified OpenAI Chat Completions ingress.",
        scope: EndpointScope::Public,
        transport: EndpointTransport::Http,
        operation: Operation::Generate,
        stream_transport: StreamTransport::Sse,
        surface_id: Some("openai_chat"),
        note: None,
    },
    EndpointSpec {
        id: "openai_completions",
        family: IngressProtocol::OpenAi,
        method: "POST",
        path: "/v1/completions",
        description: "Legacy OpenAI Completions compatibility route.",
        scope: EndpointScope::Public,
        transport: EndpointTransport::Http,
        operation: Operation::Generate,
        stream_transport: StreamTransport::Sse,
        surface_id: Some("openai_chat"),
        note: None,
    },
    EndpointSpec {
        id: "openai_responses",
        family: IngressProtocol::OpenAi,
        method: "POST",
        path: "/v1/responses",
        description: "Native OpenAI Responses passthrough for OpenAI-format upstreams.",
        scope: EndpointScope::Public,
        transport: EndpointTransport::Http,
        operation: Operation::Generate,
        stream_transport: StreamTransport::Sse,
        surface_id: Some("openai_responses"),
        note: None,
    },
    EndpointSpec {
        id: "openai_responses_ws",
        family: IngressProtocol::OpenAi,
        method: "GET",
        path: "/v1/responses/ws",
        description: "WebSocket facade over Responses SSE with create/append semantics.",
        scope: EndpointScope::Public,
        transport: EndpointTransport::WebSocket,
        operation: Operation::Generate,
        stream_transport: StreamTransport::WebSocketEvents,
        surface_id: Some("openai_responses_ws"),
        note: Some("Terminal completion is signaled by response.completed, not [DONE]."),
    },
    EndpointSpec {
        id: "openai_models",
        family: IngressProtocol::OpenAi,
        method: "GET",
        path: "/v1/models",
        description: "Gateway-local model registry for OpenAI clients.",
        scope: EndpointScope::Public,
        transport: EndpointTransport::Http,
        operation: Operation::ListModels,
        stream_transport: StreamTransport::None,
        surface_id: None,
        note: Some("Served from configured provider inventory, not upstream model listing."),
    },
    EndpointSpec {
        id: "claude_messages",
        family: IngressProtocol::Claude,
        method: "POST",
        path: "/v1/messages",
        description: "Unified Claude Messages ingress.",
        scope: EndpointScope::Public,
        transport: EndpointTransport::Http,
        operation: Operation::Generate,
        stream_transport: StreamTransport::Sse,
        surface_id: Some("claude_messages"),
        note: None,
    },
    EndpointSpec {
        id: "claude_count_tokens",
        family: IngressProtocol::Claude,
        method: "POST",
        path: "/v1/messages/count_tokens",
        description: "Direct proxy to Anthropic count_tokens for Claude-format providers.",
        scope: EndpointScope::Public,
        transport: EndpointTransport::Http,
        operation: Operation::CountTokens,
        stream_transport: StreamTransport::None,
        surface_id: Some("claude_count_tokens"),
        note: None,
    },
    EndpointSpec {
        id: "gemini_models",
        family: IngressProtocol::Gemini,
        method: "GET",
        path: "/v1beta/models",
        description: "Gateway-local Gemini model registry.",
        scope: EndpointScope::Public,
        transport: EndpointTransport::Http,
        operation: Operation::ListModels,
        stream_transport: StreamTransport::None,
        surface_id: None,
        note: Some("Served from configured provider inventory, not upstream model listing."),
    },
    EndpointSpec {
        id: "gemini_generate",
        family: IngressProtocol::Gemini,
        method: "POST",
        path: "/v1beta/models/{model}:generateContent",
        description: "Unified Gemini generateContent ingress.",
        scope: EndpointScope::Public,
        transport: EndpointTransport::Http,
        operation: Operation::Generate,
        stream_transport: StreamTransport::None,
        surface_id: Some("gemini_generate"),
        note: None,
    },
    EndpointSpec {
        id: "gemini_stream",
        family: IngressProtocol::Gemini,
        method: "POST",
        path: "/v1beta/models/{model}:streamGenerateContent",
        description: "Unified Gemini streaming ingress.",
        scope: EndpointScope::Public,
        transport: EndpointTransport::Http,
        operation: Operation::Generate,
        stream_transport: StreamTransport::Sse,
        surface_id: Some("gemini_stream"),
        note: None,
    },
    EndpointSpec {
        id: "provider_openai_chat",
        family: IngressProtocol::OpenAi,
        method: "POST",
        path: "/api/provider/{provider}/v1/chat/completions",
        description: "Provider-pinned OpenAI Chat route for deterministic routing.",
        scope: EndpointScope::ProviderScoped,
        transport: EndpointTransport::Http,
        operation: Operation::Generate,
        stream_transport: StreamTransport::Sse,
        surface_id: Some("openai_chat"),
        note: Some("Bypasses provider selection and pins requests to the named provider."),
    },
    EndpointSpec {
        id: "provider_claude_messages",
        family: IngressProtocol::Claude,
        method: "POST",
        path: "/api/provider/{provider}/v1/messages",
        description: "Provider-pinned Claude Messages route.",
        scope: EndpointScope::ProviderScoped,
        transport: EndpointTransport::Http,
        operation: Operation::Generate,
        stream_transport: StreamTransport::Sse,
        surface_id: Some("claude_messages"),
        note: Some("Bypasses provider selection and pins requests to the named provider."),
    },
    EndpointSpec {
        id: "provider_openai_responses",
        family: IngressProtocol::OpenAi,
        method: "POST",
        path: "/api/provider/{provider}/v1/responses",
        description: "Provider-pinned OpenAI Responses passthrough.",
        scope: EndpointScope::ProviderScoped,
        transport: EndpointTransport::Http,
        operation: Operation::Generate,
        stream_transport: StreamTransport::Sse,
        surface_id: Some("openai_responses"),
        note: Some("Only available for OpenAI-format providers."),
    },
    EndpointSpec {
        id: "provider_openai_responses_ws",
        family: IngressProtocol::OpenAi,
        method: "GET",
        path: "/api/provider/{provider}/v1/responses/ws",
        description: "Provider-pinned WebSocket Responses facade.",
        scope: EndpointScope::ProviderScoped,
        transport: EndpointTransport::WebSocket,
        operation: Operation::Generate,
        stream_transport: StreamTransport::WebSocketEvents,
        surface_id: Some("openai_responses_ws"),
        note: Some("Preserves Codex previous_response_id when the pinned provider is Codex."),
    },
];

/// GET /api/dashboard/protocols/matrix
/// Returns endpoint inventory and runtime provider coverage for the dashboard.
pub async fn protocol_matrix(State(state): State<AppState>) -> Json<ProtocolMatrixResponse> {
    let config = state.config.load();
    let coverage = build_protocol_coverage(&config.providers, &state);
    let active_provider_count = config
        .providers
        .iter()
        .filter(|provider| !provider.disabled)
        .count();
    let endpoints = ENDPOINT_SPECS
        .iter()
        .map(|spec| ProtocolEndpointEntry {
            id: spec.id.to_string(),
            family: spec.family,
            method: spec.method.to_string(),
            path: spec.path.to_string(),
            description: spec.description.to_string(),
            scope: spec.scope,
            transport: spec.transport,
            operation: spec.operation,
            stream_transport: spec.stream_transport,
            state: endpoint_state(spec, &coverage, active_provider_count),
            note: spec.note.map(str::to_string),
        })
        .collect();

    Json(ProtocolMatrixResponse {
        endpoints,
        coverage,
    })
}

/// GET /api/dashboard/providers/capabilities
/// Returns runtime capability truth for all providers and their models.
pub async fn provider_capabilities(
    State(state): State<AppState>,
) -> Json<ProviderCapabilitiesResponse> {
    let config = state.config.load();
    let mut providers = Vec::new();

    for provider in &config.providers {
        let upstream = provider.upstream_kind();
        let protocol = upstream_protocol_for_kind(upstream);
        let caps = default_capabilities_for_protocol(protocol);
        let probe = cached_probe_result(&state, &provider.name);
        let models = provider
            .models
            .iter()
            .map(|model| ProviderModelEntry {
                id: model.id.clone(),
                alias: model.alias.clone(),
            })
            .collect();

        providers.push(ProviderCapabilityEntry {
            name: provider.name.clone(),
            format: provider.format,
            upstream,
            upstream_protocol: protocol,
            wire_api: provider.wire_api,
            presentation_profile: provider.upstream_presentation.profile.clone(),
            presentation_mode: provider.upstream_presentation.mode.clone(),
            models,
            capabilities: caps,
            probe_status: probe
                .as_ref()
                .map(|value| value.status.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            checked_at: probe.as_ref().map(|value| value.checked_at.clone()),
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

fn build_protocol_coverage(
    providers: &[ProviderKeyEntry],
    state: &AppState,
) -> Vec<ProtocolCoverageEntry> {
    let mut coverage = Vec::new();

    for provider in providers {
        let upstream = provider.upstream_kind();
        let upstream_protocol = upstream_protocol_for_kind(upstream);
        let probe = cached_probe_result(state, &provider.name);

        for spec in SURFACE_SPECS {
            let state = surface_state(provider, probe.as_ref(), spec);
            let execution_mode = if state.status == ProbeStatus::Unsupported {
                None
            } else {
                Some(upstream_protocol.execution_mode_for(spec.ingress_protocol))
            };
            coverage.push(ProtocolCoverageEntry {
                provider: provider.name.clone(),
                format: provider.format,
                upstream,
                upstream_protocol,
                wire_api: provider.wire_api,
                disabled: provider.disabled,
                surface_id: spec.id.to_string(),
                surface_label: spec.label.to_string(),
                ingress_protocol: spec.ingress_protocol,
                execution_mode,
                state,
            });
        }
    }

    coverage
}

fn surface_state(
    provider: &ProviderKeyEntry,
    probe: Option<&ProviderProbeResult>,
    spec: &SurfaceSpec,
) -> CapabilityProbeState {
    if provider.disabled {
        return CapabilityProbeState {
            status: ProbeStatus::Unsupported,
            message: Some("provider is disabled".to_string()),
        };
    }

    if !spec.allowed_formats.contains(&provider.format) {
        let allowed = spec
            .allowed_formats
            .iter()
            .map(Format::as_str)
            .collect::<Vec<_>>()
            .join(", ");
        return CapabilityProbeState {
            status: ProbeStatus::Unsupported,
            message: Some(format!("surface requires provider format: {allowed}")),
        };
    }

    match spec.probe_kind {
        SurfaceProbeKind::Text => probe_state_ref(probe, "text"),
        SurfaceProbeKind::Stream => probe_state_ref(probe, "stream"),
        SurfaceProbeKind::CountTokens => probe_state_ref(probe, "count_tokens"),
    }
}

fn endpoint_state(
    endpoint: &EndpointSpec,
    coverage: &[ProtocolCoverageEntry],
    active_provider_count: usize,
) -> CapabilityProbeState {
    if endpoint.operation == Operation::ListModels {
        return if active_provider_count > 0 {
            CapabilityProbeState {
                status: ProbeStatus::Verified,
                message: Some("served from configured provider inventory".to_string()),
            }
        } else {
            CapabilityProbeState {
                status: ProbeStatus::Unsupported,
                message: Some("no active providers configured".to_string()),
            }
        };
    }

    let Some(surface_id) = endpoint.surface_id else {
        return CapabilityProbeState {
            status: ProbeStatus::Unknown,
            message: Some("no route state available".to_string()),
        };
    };

    let surface_entries = coverage
        .iter()
        .filter(|entry| !entry.disabled && entry.surface_id == surface_id)
        .collect::<Vec<_>>();
    if surface_entries.is_empty() {
        return CapabilityProbeState {
            status: ProbeStatus::Unsupported,
            message: Some("no active providers expose this surface".to_string()),
        };
    }

    if surface_entries
        .iter()
        .any(|entry| entry.state.status == ProbeStatus::Verified)
    {
        return CapabilityProbeState {
            status: ProbeStatus::Verified,
            message: Some("at least one active provider has verified runtime support".to_string()),
        };
    }

    if surface_entries
        .iter()
        .any(|entry| entry.state.status == ProbeStatus::Unknown)
    {
        return CapabilityProbeState {
            status: ProbeStatus::Unknown,
            message: Some(
                "surface is configured but no successful live probe has been recorded".to_string(),
            ),
        };
    }

    if surface_entries
        .iter()
        .any(|entry| entry.state.status == ProbeStatus::Failed)
    {
        return CapabilityProbeState {
            status: ProbeStatus::Failed,
            message: Some(
                "all active providers for this surface failed the live probe".to_string(),
            ),
        };
    }

    CapabilityProbeState {
        status: ProbeStatus::Unsupported,
        message: Some("surface is unsupported by all active providers".to_string()),
    }
}

fn probe_state(probe: &Option<ProviderProbeResult>, capability: &str) -> CapabilityProbeState {
    probe_state_ref(probe.as_ref(), capability)
}

fn probe_state_ref(probe: Option<&ProviderProbeResult>, capability: &str) -> CapabilityProbeState {
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

#[derive(Debug, Serialize)]
pub struct ProtocolMatrixResponse {
    pub endpoints: Vec<ProtocolEndpointEntry>,
    pub coverage: Vec<ProtocolCoverageEntry>,
}

#[derive(Debug, Serialize)]
pub struct ProtocolEndpointEntry {
    pub id: String,
    pub family: IngressProtocol,
    pub method: String,
    pub path: String,
    pub description: String,
    pub scope: EndpointScope,
    pub transport: EndpointTransport,
    pub operation: Operation,
    pub stream_transport: StreamTransport,
    pub state: CapabilityProbeState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProtocolCoverageEntry {
    pub provider: String,
    pub format: Format,
    pub upstream: UpstreamKind,
    pub upstream_protocol: UpstreamProtocol,
    pub wire_api: WireApi,
    pub disabled: bool,
    pub surface_id: String,
    pub surface_label: String,
    pub ingress_protocol: IngressProtocol,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_mode: Option<ExecutionMode>,
    pub state: CapabilityProbeState,
}

#[derive(Debug, Serialize)]
pub struct ProviderCapabilitiesResponse {
    pub providers: Vec<ProviderCapabilityEntry>,
}

#[derive(Debug, Serialize)]
pub struct ProviderModelEntry {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProviderCapabilityEntry {
    pub name: String,
    pub format: Format,
    pub upstream: UpstreamKind,
    pub upstream_protocol: UpstreamProtocol,
    pub wire_api: WireApi,
    pub presentation_profile: ProfileKind,
    pub presentation_mode: ActivationMode,
    pub models: Vec<ProviderModelEntry>,
    pub capabilities: ProviderCapabilities,
    pub probe_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked_at: Option<String>,
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
