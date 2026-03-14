use serde::{Deserialize, Serialize};

/// A tool specification describing a callable function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub parameters: serde_json::Value,
}

/// How the model should choose tools.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolChoice {
    /// Let the model decide.
    #[default]
    Auto,
    /// Never use tools.
    None,
    /// Must use at least one tool.
    Required,
    /// Must use the specified tool.
    Tool { name: String },
}

/// Requested response format.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseFormat {
    /// Unstructured text output.
    #[default]
    Text,
    /// JSON output (optionally with a schema).
    JsonSchema {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        schema: Option<serde_json::Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default)]
        strict: bool,
    },
    /// Raw JSON mode without schema.
    JsonObject,
}
