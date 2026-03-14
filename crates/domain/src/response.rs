use serde::{Deserialize, Serialize};

use crate::content::ContentBlock;
use crate::operation::ExecutionMode;

/// Canonical response from a provider execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalResponse {
    /// Unique response ID.
    pub id: String,
    /// Model that generated the response.
    pub model: String,
    /// The response content blocks.
    pub content: Vec<ContentBlock>,
    /// Stop reason.
    pub stop_reason: StopReason,
    /// Token usage.
    pub usage: Usage,
    /// How this response was produced.
    pub execution_mode: ExecutionMode,
    /// Provider that served the request.
    pub provider: String,
    /// Credential that was used.
    pub credential: String,
}

/// Why the model stopped generating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// Natural end of generation.
    EndTurn,
    /// Hit a stop sequence.
    StopSequence,
    /// Reached max tokens limit.
    MaxTokens,
    /// Model decided to use a tool.
    ToolUse,
}

/// Token usage information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_read_tokens: u64,
    #[serde(default)]
    pub cache_creation_tokens: u64,
    #[serde(default)]
    pub reasoning_tokens: u64,
}

/// Canonical count-tokens response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountTokensResponse {
    pub input_tokens: u64,
}
