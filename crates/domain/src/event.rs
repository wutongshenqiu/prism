use serde::{Deserialize, Serialize};

use crate::content::ContentBlock;
use crate::response::{StopReason, Usage};

/// A canonical streaming event emitted by the runtime.
/// Protocol egress adapters translate these into protocol-specific SSE events.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CanonicalEvent {
    /// Stream has started — contains metadata.
    StreamStart { id: String, model: String },

    /// A new content block has started.
    ContentBlockStart { index: u32, block: ContentBlock },

    /// Incremental text delta within a content block.
    TextDelta { index: u32, text: String },

    /// Incremental thinking text delta.
    ThinkingDelta { index: u32, thinking: String },

    /// Incremental tool input JSON delta.
    ToolInputDelta { index: u32, partial_json: String },

    /// A content block has finished.
    ContentBlockStop { index: u32 },

    /// Final usage and stop reason.
    StreamEnd {
        stop_reason: StopReason,
        usage: Usage,
    },

    /// Keepalive / ping event.
    Ping,
}
