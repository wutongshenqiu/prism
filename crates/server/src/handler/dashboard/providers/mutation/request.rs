use prism_core::auth_profile::AuthProfileEntry;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct CreateProviderRequest {
    pub name: String,
    pub format: String,
    #[serde(default)]
    pub upstream: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub auth_profiles: Vec<AuthProfileEntry>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub proxy_url: Option<String>,
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub excluded_models: Vec<String>,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub wire_api: Option<String>,
    #[serde(default = "default_weight")]
    pub weight: u32,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub upstream_presentation: Option<prism_core::presentation::UpstreamPresentationConfig>,
    #[serde(default)]
    pub vertex: bool,
    #[serde(default)]
    pub vertex_project: Option<String>,
    #[serde(default)]
    pub vertex_location: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct UpdateProviderRequest {
    #[serde(default)]
    pub upstream: Option<Option<String>>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub auth_profiles: Option<Vec<AuthProfileEntry>>,
    #[serde(default)]
    pub base_url: Option<Option<String>>,
    #[serde(default)]
    pub proxy_url: Option<Option<String>>,
    #[serde(default)]
    pub prefix: Option<Option<String>>,
    #[serde(default)]
    pub models: Option<Vec<String>>,
    #[serde(default)]
    pub excluded_models: Option<Vec<String>>,
    #[serde(default)]
    pub headers: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    pub disabled: Option<bool>,
    #[serde(default)]
    pub wire_api: Option<Option<String>>,
    #[serde(default)]
    pub weight: Option<u32>,
    #[serde(default)]
    pub region: Option<Option<String>>,
    #[serde(default)]
    pub upstream_presentation: Option<Option<prism_core::presentation::UpstreamPresentationConfig>>,
    #[serde(default)]
    pub vertex: Option<bool>,
    #[serde(default)]
    pub vertex_project: Option<Option<String>>,
    #[serde(default)]
    pub vertex_location: Option<Option<String>>,
}

fn default_weight() -> u32 {
    1
}
