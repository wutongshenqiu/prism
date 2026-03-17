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
pub struct FactRow {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InspectorRow {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InspectorSection {
    pub title: String,
    pub rows: Vec<InspectorRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceInspector {
    pub eyebrow: String,
    pub title: String,
    pub summary: String,
    pub sections: Vec<InspectorSection>,
    pub actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KpiMetric {
    pub label: String,
    pub value: String,
    pub delta: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SignalItem {
    pub id: String,
    pub title: String,
    pub detail: String,
    pub severity: String,
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
    pub decision: String,
    pub result: String,
    pub result_tone: String,
    pub latency_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimelineStep {
    pub label: String,
    pub tone: String,
    pub title: String,
    pub detail: String,
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
    pub auth: String,
    pub status: String,
    pub status_tone: String,
    pub rotation: String,
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
    pub decision: String,
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
    pub record: String,
    pub state: String,
    pub state_tone: String,
    pub dependents: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChangeStudioResponse {
    pub registry: Vec<RegistryRow>,
    pub publish_facts: Vec<FactRow>,
    pub inspector: WorkspaceInspector,
}
