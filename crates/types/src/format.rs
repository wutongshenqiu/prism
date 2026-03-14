use serde::{Deserialize, Serialize};

/// Supported wire protocol format identifiers.
///
/// Represents the JSON request/response structure used by the upstream API.
/// Provider identity (e.g., OpenAI vs DeepSeek) is determined by the
/// provider `name` in config, not by this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Format {
    #[serde(rename = "openai")]
    OpenAI,
    Claude,
    Gemini,
}

impl Format {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OpenAI => "openai",
            Self::Claude => "claude",
            Self::Gemini => "gemini",
        }
    }

    /// Canonical default base URL for this wire protocol.
    pub fn default_base_url(&self) -> &'static str {
        match self {
            Self::OpenAI => "https://api.openai.com",
            Self::Claude => "https://api.anthropic.com",
            Self::Gemini => "https://generativelanguage.googleapis.com",
        }
    }
}

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Format {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "openai" => Ok(Self::OpenAI),
            "claude" => Ok(Self::Claude),
            "gemini" => Ok(Self::Gemini),
            _ => Err(format!("unknown format: {s}")),
        }
    }
}

/// Wire API format for OpenAI-compatible providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WireApi {
    #[default]
    Chat,
    Responses,
}
