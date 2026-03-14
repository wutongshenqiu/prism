use serde::{Deserialize, Serialize};

/// The public ingress protocol used by the client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IngressProtocol {
    OpenAi,
    Claude,
    Gemini,
}

impl std::fmt::Display for IngressProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OpenAi => f.write_str("openai"),
            Self::Claude => f.write_str("claude"),
            Self::Gemini => f.write_str("gemini"),
        }
    }
}

/// The high-level operation requested by the client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    /// Generate a completion (chat, messages, generateContent).
    Generate,
    /// Count tokens for a request without executing it.
    CountTokens,
    /// List available models.
    ListModels,
}

/// The specific public endpoint that received the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Endpoint {
    /// POST /v1/chat/completions
    ChatCompletions,
    /// POST /v1/responses
    Responses,
    /// POST /v1/messages
    Messages,
    /// POST /v1/messages/count_tokens
    MessagesCountTokens,
    /// POST /v1beta/models/{model}:generateContent
    GenerateContent,
    /// POST /v1beta/models/{model}:streamGenerateContent
    StreamGenerateContent,
    /// GET /v1/models or /v1beta/models
    Models,
}

impl Endpoint {
    pub fn operation(&self) -> Operation {
        match self {
            Self::ChatCompletions | Self::Responses | Self::Messages => Operation::Generate,
            Self::GenerateContent | Self::StreamGenerateContent => Operation::Generate,
            Self::MessagesCountTokens => Operation::CountTokens,
            Self::Models => Operation::ListModels,
        }
    }

    pub fn ingress_protocol(&self) -> IngressProtocol {
        match self {
            Self::ChatCompletions | Self::Responses | Self::Models => IngressProtocol::OpenAi,
            Self::Messages | Self::MessagesCountTokens => IngressProtocol::Claude,
            Self::GenerateContent | Self::StreamGenerateContent => IngressProtocol::Gemini,
        }
    }
}

/// How the provider will execute this request relative to the ingress protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    /// Provider speaks the same protocol as the ingress — no translation needed.
    Native,
    /// Request is translated losslessly to the provider's native protocol.
    LosslessAdapted,
}
