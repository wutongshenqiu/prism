use serde::{Deserialize, Serialize};

use crate::operation::{ExecutionMode, IngressProtocol};
use crate::request::RequiredCapabilities;

/// Declared capabilities of a provider or model.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    #[serde(default)]
    pub supports_stream: bool,
    #[serde(default)]
    pub supports_tools: bool,
    #[serde(default)]
    pub supports_parallel_tools: bool,
    #[serde(default)]
    pub supports_json_schema: bool,
    #[serde(default)]
    pub supports_reasoning: bool,
    #[serde(default)]
    pub supports_images: bool,
    #[serde(default)]
    pub supports_count_tokens: bool,
}

impl ProviderCapabilities {
    /// Returns the list of capabilities that are required but not supported.
    pub fn missing_capabilities(&self, required: &RequiredCapabilities) -> Vec<String> {
        let mut missing = Vec::new();
        if required.supports_stream && !self.supports_stream {
            missing.push("supports_stream".into());
        }
        if required.supports_tools && !self.supports_tools {
            missing.push("supports_tools".into());
        }
        if required.supports_parallel_tools && !self.supports_parallel_tools {
            missing.push("supports_parallel_tools".into());
        }
        if required.supports_json_schema && !self.supports_json_schema {
            missing.push("supports_json_schema".into());
        }
        if required.supports_reasoning && !self.supports_reasoning {
            missing.push("supports_reasoning".into());
        }
        if required.supports_images && !self.supports_images {
            missing.push("supports_images".into());
        }
        if required.supports_count_tokens && !self.supports_count_tokens {
            missing.push("supports_count_tokens".into());
        }
        missing
    }

    /// Check whether all required capabilities are satisfied.
    pub fn satisfies(&self, required: &RequiredCapabilities) -> bool {
        (!required.supports_stream || self.supports_stream)
            && (!required.supports_tools || self.supports_tools)
            && (!required.supports_parallel_tools || self.supports_parallel_tools)
            && (!required.supports_json_schema || self.supports_json_schema)
            && (!required.supports_reasoning || self.supports_reasoning)
            && (!required.supports_images || self.supports_images)
            && (!required.supports_count_tokens || self.supports_count_tokens)
    }
}

/// Which upstream protocol a provider speaks natively.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpstreamProtocol {
    OpenAi,
    Anthropic,
    Gemini,
}

impl UpstreamProtocol {
    /// Determine the execution mode when serving the given ingress protocol.
    pub fn execution_mode_for(&self, ingress: IngressProtocol) -> ExecutionMode {
        match (ingress, self) {
            (IngressProtocol::OpenAi, UpstreamProtocol::OpenAi) => ExecutionMode::Native,
            (IngressProtocol::Claude, UpstreamProtocol::Anthropic) => ExecutionMode::Native,
            (IngressProtocol::Gemini, UpstreamProtocol::Gemini) => ExecutionMode::Native,
            _ => ExecutionMode::LosslessAdapted,
        }
    }
}

/// Full capability declaration for a provider-model combination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCapabilityEntry {
    pub provider_name: String,
    pub model_id: String,
    pub upstream_protocol: UpstreamProtocol,
    pub capabilities: ProviderCapabilities,
}

/// Default capabilities inferred from an upstream protocol.
/// These serve as sensible defaults when the user doesn't declare capabilities.
pub fn default_capabilities_for_protocol(protocol: UpstreamProtocol) -> ProviderCapabilities {
    match protocol {
        UpstreamProtocol::OpenAi => ProviderCapabilities {
            supports_stream: true,
            supports_tools: true,
            supports_parallel_tools: true,
            supports_json_schema: true,
            supports_reasoning: false,
            supports_images: true,
            supports_count_tokens: false,
        },
        UpstreamProtocol::Anthropic => ProviderCapabilities {
            supports_stream: true,
            supports_tools: true,
            supports_parallel_tools: false,
            supports_json_schema: false,
            supports_reasoning: true,
            supports_images: true,
            supports_count_tokens: true,
        },
        UpstreamProtocol::Gemini => ProviderCapabilities {
            supports_stream: true,
            supports_tools: true,
            supports_parallel_tools: false,
            supports_json_schema: true,
            supports_reasoning: true,
            supports_images: true,
            supports_count_tokens: true,
        },
    }
}
