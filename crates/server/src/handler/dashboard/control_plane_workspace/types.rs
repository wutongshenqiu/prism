use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

const DEFAULT_TRAFFIC_LIMIT: usize = 12;

#[derive(Debug, Clone, Deserialize)]
pub struct WorkspaceQuery {
    #[serde(default = "default_range")]
    pub range: String,
    #[serde(default = "default_source_mode")]
    pub source_mode: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_range() -> String {
    "1h".to_string()
}

fn default_source_mode() -> String {
    "hybrid".to_string()
}

fn default_limit() -> usize {
    DEFAULT_TRAFFIC_LIMIT
}

#[derive(Debug, Clone, Serialize)]
pub struct UiText {
    pub key: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub values: BTreeMap<String, String>,
}

impl UiText {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            values: BTreeMap::new(),
        }
    }

    pub fn with_values<I, K, V>(key: impl Into<String>, values: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: ToString,
    {
        Self {
            key: key.into(),
            values: values
                .into_iter()
                .map(|(key, value)| (key.into(), value.to_string()))
                .collect(),
        }
    }
}

pub fn raw_text(value: impl ToString) -> UiText {
    UiText::with_values("common.raw", [("value", value.to_string())])
}

#[derive(Debug, Clone, Serialize)]
pub struct FactRow {
    pub label: UiText,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_text: Option<UiText>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InspectorRow {
    pub label: UiText,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_text: Option<UiText>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InspectorSection {
    pub title: UiText,
    pub rows: Vec<InspectorRow>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceActionEffect {
    Navigate,
    Reload,
    Invoke,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceAction {
    pub id: String,
    pub label: UiText,
    pub effect: WorkspaceActionEffect,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_workspace: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceInspector {
    pub eyebrow: UiText,
    pub title: UiText,
    pub summary: UiText,
    pub sections: Vec<InspectorSection>,
    pub actions: Vec<WorkspaceAction>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KpiMetric {
    pub label: UiText,
    pub value: String,
    pub delta: UiText,
}

#[derive(Debug, Clone, Serialize)]
pub struct SignalItem {
    pub id: String,
    pub title: UiText,
    pub detail: UiText,
    pub severity: UiText,
    pub severity_tone: String,
    pub target_workspace: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommandCenterResponse {
    pub kpis: Vec<KpiMetric>,
    pub signals: Vec<SignalItem>,
    pub pressure_map: Vec<FactRow>,
    pub watch_windows: Vec<FactRow>,
    pub inspector: WorkspaceInspector,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrafficSessionItem {
    pub request_id: String,
    pub model: String,
    pub decision: UiText,
    pub result: UiText,
    pub result_tone: String,
    pub latency_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimelineStep {
    pub label: UiText,
    pub tone: String,
    pub title: UiText,
    pub detail: UiText,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrafficLabResponse {
    pub selected_request_id: Option<String>,
    pub sessions: Vec<TrafficSessionItem>,
    pub compare_facts: Vec<FactRow>,
    pub trace: Vec<TimelineStep>,
    pub inspector: WorkspaceInspector,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderAtlasRow {
    pub provider: String,
    pub format: String,
    pub auth: UiText,
    pub status: UiText,
    pub status_tone: String,
    pub rotation: UiText,
    pub region: String,
    pub wire_api: String,
    pub model_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderAtlasResponse {
    pub providers: Vec<ProviderAtlasRow>,
    pub coverage: Vec<FactRow>,
    pub inspector: WorkspaceInspector,
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteScenarioRow {
    pub scenario: String,
    pub winner: String,
    pub delta: String,
    pub decision: UiText,
    pub decision_tone: String,
    pub endpoint: String,
    pub source_format: String,
    pub stream: bool,
    pub model: String,
    pub tenant_id: Option<String>,
    pub api_key_id: Option<String>,
    pub region: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteStudioResponse {
    pub summary_facts: Vec<FactRow>,
    pub explain_facts: Vec<FactRow>,
    pub scenarios: Vec<RouteScenarioRow>,
    pub inspector: WorkspaceInspector,
}

#[derive(Debug, Clone, Serialize)]
pub struct RegistryRow {
    pub family: String,
    pub family_label: UiText,
    pub record: String,
    pub state: UiText,
    pub state_tone: String,
    pub dependents: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChangeStudioResponse {
    pub registry: Vec<RegistryRow>,
    pub publish_facts: Vec<FactRow>,
    pub inspector: WorkspaceInspector,
}
