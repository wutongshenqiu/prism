import type { LocalizedText } from './i18n';
import type { ShellInspectorState, SourceMode, TimeRangeMode, WorkspaceId } from './shell';

export type StatusTone = 'neutral' | 'success' | 'warning' | 'danger' | 'info';

export interface WorkspaceQuery {
  range: TimeRangeMode;
  sourceMode: SourceMode;
}

export interface FactRow {
  label: LocalizedText;
  value: string;
  value_text?: LocalizedText;
}

export interface KpiMetric {
  label: LocalizedText;
  value: string;
  delta: LocalizedText;
}

export interface SignalItem {
  id: string;
  title: LocalizedText;
  detail: LocalizedText;
  severity: LocalizedText;
  severity_tone: StatusTone;
  target_workspace: WorkspaceId;
}

export interface CommandCenterResponse {
  kpis: KpiMetric[];
  signals: SignalItem[];
  pressure_map: FactRow[];
  watch_windows: FactRow[];
  inspector: ShellInspectorState;
}

export interface TrafficSessionItem {
  request_id: string;
  model: string;
  decision: LocalizedText;
  result: LocalizedText;
  result_tone: StatusTone;
  latency_ms: number;
}

export interface TimelineStep {
  label: LocalizedText;
  tone: StatusTone;
  title: LocalizedText;
  detail: LocalizedText;
}

export interface TrafficLabResponse {
  selected_request_id: string | null;
  sessions: TrafficSessionItem[];
  compare_facts: FactRow[];
  trace: TimelineStep[];
  inspector: ShellInspectorState;
}

export interface ProviderAtlasRow {
  provider: string;
  format: string;
  auth: LocalizedText;
  status: LocalizedText;
  status_tone: StatusTone;
  rotation: LocalizedText;
  region: string;
  wire_api: string;
  model_count: number;
}

export interface ProviderAtlasResponse {
  providers: ProviderAtlasRow[];
  coverage: FactRow[];
  inspector: ShellInspectorState;
}

export interface RouteScenarioRow {
  scenario: string;
  winner: string;
  delta: string;
  decision: LocalizedText;
  decision_tone: StatusTone;
  endpoint: string;
  source_format: string;
  stream: boolean;
  model: string;
  tenant_id: string | null;
  api_key_id: string | null;
  region: string | null;
}

export interface RouteStudioResponse {
  summary_facts: FactRow[];
  explain_facts: FactRow[];
  scenarios: RouteScenarioRow[];
  inspector: ShellInspectorState;
}

export interface RegistryRow {
  family: string;
  family_label: LocalizedText;
  record: string;
  state: LocalizedText;
  state_tone: StatusTone;
  dependents: string;
}

export interface ChangeStudioResponse {
  registry: RegistryRow[];
  publish_facts: FactRow[];
  inspector: ShellInspectorState;
}
