use serde::{Deserialize, Serialize};

use crate::content::Conversation;
use crate::operation::{Endpoint, ExecutionMode, IngressProtocol, Operation};
use crate::tool::{ResponseFormat, ToolChoice, ToolSpec};

/// The canonical runtime request — protocol-agnostic representation
/// of any public inference request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalRequest {
    /// Which public protocol the client used.
    pub ingress_protocol: IngressProtocol,
    /// The high-level operation.
    pub operation: Operation,
    /// The specific endpoint that received this request.
    pub endpoint: Endpoint,
    /// Requested model identifier.
    pub model: String,
    /// Whether streaming was requested.
    pub stream: bool,

    // ── Content ──────────────────────────────────────────────────────
    /// The conversation (system + messages).
    pub input: Conversation,
    /// Tool definitions.
    #[serde(default)]
    pub tools: Vec<ToolSpec>,
    /// Tool choice strategy.
    #[serde(default)]
    pub tool_choice: ToolChoice,
    /// Requested response format.
    #[serde(default)]
    pub response_format: ResponseFormat,

    // ── Reasoning ────────────────────────────────────────────────────
    /// Extended thinking / reasoning configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<ReasoningConfig>,

    // ── Limits ───────────────────────────────────────────────────────
    pub limits: RequestLimits,

    // ── Routing context ──────────────────────────────────────────────
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,

    /// Original raw body bytes (preserved for native passthrough).
    #[serde(skip)]
    pub raw_body: Option<bytes::Bytes>,
}

/// Reasoning / extended thinking configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningConfig {
    /// Whether thinking is enabled.
    pub enabled: bool,
    /// Budget tokens for thinking (Claude) or reasoning effort.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_tokens: Option<u64>,
    /// Thinking effort level (e.g., "low", "medium", "high" for OpenAI).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
}

/// Token and generation limits.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RequestLimits {
    /// Maximum tokens to generate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u64>,
    /// Sampling temperature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// Top-p sampling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    /// Top-k sampling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    /// Stop sequences.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stop: Vec<String>,
}

/// The required capabilities derived from a CanonicalRequest.
/// Used by the planner to filter candidate providers.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RequiredCapabilities {
    pub supports_generate: bool,
    pub supports_stream: bool,
    pub supports_tools: bool,
    pub supports_parallel_tools: bool,
    pub supports_json_schema: bool,
    pub supports_reasoning: bool,
    pub supports_images: bool,
    pub supports_count_tokens: bool,
}

impl CanonicalRequest {
    /// Derive the set of capabilities that a provider must have
    /// in order to serve this request.
    pub fn required_capabilities(&self) -> RequiredCapabilities {
        let has_images = self.input.messages.iter().any(|m| {
            m.content
                .iter()
                .any(|c| matches!(c, crate::content::ContentBlock::Image { .. }))
        });

        RequiredCapabilities {
            supports_generate: self.operation == Operation::Generate,
            supports_stream: self.stream,
            supports_tools: !self.tools.is_empty(),
            supports_parallel_tools: false, // derived from tool_choice if needed
            supports_json_schema: matches!(self.response_format, ResponseFormat::JsonSchema { .. }),
            supports_reasoning: self.reasoning.as_ref().is_some_and(|r| r.enabled),
            supports_images: has_images,
            supports_count_tokens: self.operation == Operation::CountTokens,
        }
    }
}

/// A planner explain request — used by the control plane
/// to explain routing decisions without executing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplainRequest {
    pub ingress_protocol: IngressProtocol,
    pub operation: Operation,
    pub endpoint: Endpoint,
    pub model: String,
    pub stream: bool,
    /// Feature flags instead of full content.
    #[serde(default)]
    pub features: ExplainFeatures,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
}

/// Lightweight feature flags for explain requests.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExplainFeatures {
    #[serde(default)]
    pub tools: bool,
    #[serde(default)]
    pub json_schema: bool,
    #[serde(default)]
    pub reasoning: bool,
    #[serde(default)]
    pub images: bool,
    #[serde(default)]
    pub count_tokens: bool,
}

impl ExplainRequest {
    /// Convert explain features to required capabilities.
    pub fn required_capabilities(&self) -> RequiredCapabilities {
        RequiredCapabilities {
            supports_generate: self.operation == Operation::Generate,
            supports_stream: self.stream,
            supports_tools: self.features.tools,
            supports_parallel_tools: false,
            supports_json_schema: self.features.json_schema,
            supports_reasoning: self.features.reasoning,
            supports_images: self.features.images,
            supports_count_tokens: self.features.count_tokens,
        }
    }
}

/// Planner explain response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplainResponse {
    /// The selected route (if any).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<SelectedRoute>,
    /// Alternate candidates that could serve this request.
    #[serde(default)]
    pub alternates: Vec<SelectedRoute>,
    /// Candidates that were rejected and why.
    #[serde(default)]
    pub rejections: Vec<Rejection>,
    /// The capabilities that were required.
    pub required_capabilities: RequiredCapabilities,
}

/// A selected route for execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectedRoute {
    pub provider: String,
    pub credential: String,
    pub model: String,
    pub execution_mode: ExecutionMode,
}

/// A rejected candidate with a structured reason.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rejection {
    pub candidate: String,
    pub reason: RejectionReason,
}

/// Structured rejection reason.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RejectionReason {
    MissingCapability { capability: String },
    ModelNotSupported { model: String },
    CredentialUnavailable,
    RegionMismatch { required: String, actual: String },
    TenantNotAllowed,
    CircuitOpen,
    Disabled,
}
