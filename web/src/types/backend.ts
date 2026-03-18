export interface LoginResponse {
  authenticated: boolean;
  username: string;
  expires_in: number;
}

export interface SessionResponse {
  authenticated: boolean;
  username: string;
}

export interface TokenUsage {
  input_tokens: number;
  output_tokens: number;
  cache_read_tokens: number;
  cache_creation_tokens: number;
}

export interface AttemptSummary {
  attempt_index: number;
  provider: string;
  model: string;
  credential_name: string | null;
  status: number | null;
  latency_ms: number;
  error: string | null;
  error_type: string | null;
}

export interface RequestLog {
  request_id: string;
  timestamp: string;
  method: string;
  path: string;
  stream: boolean;
  requested_model: string | null;
  provider: string | null;
  model: string | null;
  credential_name: string | null;
  total_attempts: number;
  status: number;
  latency_ms: number;
  request_body?: string | null;
  upstream_request_body?: string | null;
  response_body?: string | null;
  stream_content_preview?: string | null;
  usage: TokenUsage | null;
  cost: number | null;
  error?: string | null;
  error_type?: string | null;
  api_key_id: string | null;
  tenant_id: string | null;
  client_ip: string | null;
  client_region?: string | null;
  attempts?: AttemptSummary[];
}

export interface ProviderAuthProfile {
  id: string;
  qualified_name: string;
  mode: string;
  header: string;
  secret_masked?: string | null;
  access_token_masked?: string | null;
  refresh_token_present: boolean;
  id_token_present: boolean;
  expires_at?: string | null;
  account_id?: string | null;
  email?: string | null;
  last_refresh?: string | null;
  headers?: Record<string, string>;
  disabled: boolean;
  weight: number;
  region?: string | null;
  prefix?: string | null;
}

export interface ProviderDetail {
  name: string;
  format: 'openai' | 'claude' | 'gemini';
  upstream: string;
  api_key_masked: string;
  base_url: string | null;
  proxy_url: string | null;
  prefix: string | null;
  models: Array<{ id: string; alias: string | null }>;
  excluded_models: string[];
  headers: Record<string, string>;
  disabled: boolean;
  wire_api: 'chat' | 'responses';
  weight: number;
  region: string | null;
  auth_profiles: ProviderAuthProfile[];
}

export interface ProviderProbeCheck {
  capability: string;
  status: 'verified' | 'failed' | 'unknown' | 'unsupported';
  message?: string | null;
}

export interface ProviderHealthResult {
  provider: string;
  upstream: string;
  status: 'ok' | 'warning' | 'error';
  checked_at: string;
  latency_ms: number;
  checks: ProviderProbeCheck[];
}

export interface ProviderTestResponse {
  provider: string;
  upstream: string;
  endpoint: string;
  format: 'openai' | 'claude' | 'gemini';
  model: string;
  status: number;
  ok: boolean;
  latency_ms: number;
  request_body: unknown;
  response_body: unknown;
}

export interface ProviderFetchModelsResult {
  models: string[];
  supported: boolean;
  message?: string | null;
}

export interface RouteScore {
  weight: number;
  latency_ms?: number;
  inflight?: number;
  estimated_cost?: number;
  health_penalty: number;
}

export interface SelectedRoute {
  provider: string;
  credential_name: string;
  model: string;
  score: RouteScore;
}

export type RejectReason =
  | 'model_not_supported'
  | 'region_mismatch'
  | 'provider_pin_excluded'
  | 'circuit_breaker_open'
  | 'outlier_ejected'
  | 'credential_disabled'
  | 'access_denied'
  | 'cooldown_active'
  | { missing_capability: { capabilities: string[] } };

export interface RouteRejection {
  candidate: string;
  reason: RejectReason;
}

export interface ModelResolutionStep {
  step: string;
  from?: string;
  to?: string;
  rule?: string;
  primary?: string;
  fallbacks?: string[];
  model?: string;
  providers?: string[];
}

export interface RouteScoringEntry {
  candidate: string;
  score: RouteScore;
  rank: number;
}

export interface RouteExplanation {
  profile: string;
  matched_rule: string | null;
  model_chain: string[];
  selected: SelectedRoute | null;
  alternates: SelectedRoute[];
  rejections: RouteRejection[];
  model_resolution: ModelResolutionStep[];
  scoring: RouteScoringEntry[];
}

export interface RawConfigResponse {
  content: string;
  path: string;
  config_version?: string;
}

export interface ConfigValidateResponse {
  valid: boolean;
  errors: string[];
}

export interface ConfigApplyResponse {
  message: string;
  config_version: string;
}

export interface AuthProfileSummary {
  provider: string;
  format: string;
  id: string;
  qualified_name: string;
  mode: string;
  header: string;
  connected: boolean;
  secret_masked?: string | null;
  access_token_masked?: string | null;
  refresh_token_present: boolean;
  id_token_present: boolean;
  expires_at?: string | null;
  account_id?: string | null;
  email?: string | null;
  last_refresh?: string | null;
  headers: Record<string, string>;
  disabled: boolean;
  weight: number;
  region?: string | null;
  prefix?: string | null;
}

export interface AuthProfilesRuntimeResponse {
  storage_dir: string | null;
  codex_auth_file: string | null;
  proxy_url: string | null;
}

export interface AuthProfileMutationResponse {
  profile: AuthProfileSummary;
}

export interface AuthProfilesListResponse {
  profiles: AuthProfileSummary[];
}

export interface CodexOauthStartRequest {
  provider: string;
  profile_id: string;
  redirect_uri: string;
}

export interface CodexOauthStartResponse {
  state: string;
  auth_url: string;
  provider: string;
  profile_id: string;
  expires_in: number;
}

export interface CodexDeviceStartRequest {
  provider: string;
  profile_id: string;
}

export interface CodexDeviceStartResponse {
  state: string;
  provider: string;
  profile_id: string;
  verification_url: string;
  user_code: string;
  interval_secs: number;
  expires_in: number;
}

export interface CodexDevicePollResponse {
  status: 'pending' | 'completed';
  interval_secs?: number;
  profile?: AuthProfileSummary;
}

export interface AuthProfileCreateRequest {
  provider: string;
  id: string;
  mode: string;
  header?: string;
  secret?: string | null;
  headers?: Record<string, string>;
  disabled?: boolean;
  weight?: number;
  region?: string | null;
  prefix?: string | null;
}

export interface AuthProfileConnectRequest {
  secret: string;
}

export interface AuthKeySummary {
  id: number;
  key_masked: string;
  name?: string | null;
  tenant_id?: string | null;
  allowed_models: string[];
  allowed_credentials: string[];
  rate_limit: KeyRateLimitConfig | null;
  budget: BudgetConfig | null;
  expires_at?: string | null;
  metadata: Record<string, string>;
}

export interface AuthKeysResponse {
  auth_keys: AuthKeySummary[];
}

export interface KeyRateLimitConfig {
  rpm?: number;
  tpm?: number;
  cost_per_day_usd?: number;
}

export interface BudgetConfig {
  total_usd: number;
  period: 'daily' | 'monthly';
}

export interface AuthKeyCreateResponse {
  key: string;
  message: string;
}

export interface AuthKeyRevealResponse {
  key: string;
}

export interface AuthKeyCreateRequest {
  name?: string;
  tenant_id?: string;
  allowed_models?: string[];
  allowed_credentials?: string[];
  rate_limit?: KeyRateLimitConfig;
  budget?: BudgetConfig;
  expires_at?: string;
}

export interface AuthKeyUpdateRequest {
  name?: string;
  tenant_id?: string | null;
  allowed_models?: string[];
  allowed_credentials?: string[];
  rate_limit?: KeyRateLimitConfig | null;
  budget?: BudgetConfig | null;
  expires_at?: string | null;
}

export type ProviderStrategy =
  | 'ordered-fallback'
  | 'weighted-round-robin'
  | 'ewma-latency'
  | 'lowest-estimated-cost'
  | 'sticky-hash';

export type CredentialStrategy =
  | 'priority-weighted-rr'
  | 'fill-first'
  | 'least-inflight'
  | 'ewma-latency'
  | 'sticky-hash'
  | 'random-two-choices';

export interface RouteProfile {
  'provider-policy': {
    strategy: ProviderStrategy;
    'sticky-key'?: string;
    weights?: Record<string, number>;
    order?: string[];
  };
  'credential-policy': {
    strategy: CredentialStrategy;
  };
  health: {
    'circuit-breaker': {
      enabled: boolean;
      'failure-threshold': number;
      'cooldown-seconds': number;
    };
    'outlier-detection': {
      'consecutive-5xx': number;
      'consecutive-local-failures': number;
      'base-eject-seconds': number;
      'max-eject-seconds': number;
    };
  };
  failover: {
    'credential-attempts': number;
    'provider-attempts': number;
    'model-attempts': number;
    'retry-budget': {
      ratio: number;
      'min-retries-per-second': number;
    };
    'retry-on': string[];
  };
}

export interface RouteRule {
  name: string;
  priority?: number;
  match: {
    models?: string[];
    tenants?: string[];
    endpoints?: string[];
    regions?: string[];
    stream?: boolean;
    headers?: Record<string, string[]>;
  };
  'use-profile': string;
}

export interface ModelResolution {
  aliases?: Array<{ from: string; to: string }>;
  rewrites?: Array<{ pattern: string; to: string }>;
  fallbacks?: Array<{ pattern: string; to: string[] }>;
  'provider-pins'?: Array<{ pattern: string; providers: string[] }>;
}

export interface RoutingConfig {
  'default-profile': string;
  profiles: Record<string, RouteProfile>;
  rules: RouteRule[];
  'model-resolution': ModelResolution;
}

export interface RoutingUpdateRequest {
  'default-profile'?: string;
  profiles?: Record<string, RouteProfile>;
  rules?: RouteRule[];
  'model-resolution'?: ModelResolution;
}

export interface TenantSummary {
  id: string;
  requests: number;
  tokens: number;
  cost_usd: number;
}

export interface TenantsResponse {
  tenants: TenantSummary[];
}

export interface TenantMetricsResponse {
  tenant_id: string;
  metrics: {
    requests: number;
    tokens: number;
    cost_usd: number;
  } | null;
}

export interface SystemHealthProvider {
  name: string;
  status: string;
  active_keys: number;
  total_keys: number;
}

export interface SystemHealthResponse {
  status: string;
  version: string;
  uptime_seconds: number;
  host: string;
  port: number;
  tls_enabled: boolean;
  providers: SystemHealthProvider[];
  metrics: Record<string, unknown>;
}

export interface SystemLogEntry {
  timestamp: string;
  level: string;
  target: string;
  message: string;
}

export interface SystemLogsResponse {
  logs: SystemLogEntry[];
  total: number;
  page: number;
  page_size: number;
  file?: string;
  truncated?: boolean;
  message?: string;
}

export interface ProviderCapabilityProbeState {
  status: string;
  message?: string | null;
}

export interface ProviderCapabilityProbeStates {
  text: ProviderCapabilityProbeState;
  stream: ProviderCapabilityProbeState;
  tools: ProviderCapabilityProbeState;
  images: ProviderCapabilityProbeState;
  json_schema: ProviderCapabilityProbeState;
  reasoning: ProviderCapabilityProbeState;
  count_tokens: ProviderCapabilityProbeState;
}

export interface ProviderCapabilityEntry {
  name: string;
  format: string;
  upstream: string;
  upstream_protocol: string;
  wire_api: string;
  presentation_profile: string;
  presentation_mode: string;
  models: Array<{ id: string; alias: string | null }>;
  capabilities: Record<string, boolean | string | number | null>;
  probe_status: string;
  checked_at?: string | null;
  probe: ProviderCapabilityProbeStates;
  disabled: boolean;
}

export interface ProviderCapabilitiesResponse {
  providers: ProviderCapabilityEntry[];
}

export interface PresentationPreviewResponse {
  profile: string;
  activated: boolean;
  effective_headers: Record<string, string>;
  body_mutations: Array<{ kind: string; applied: boolean; reason?: string }>;
  protected_headers_blocked: string[];
  effective_body: unknown;
}

export interface ProviderCreateRequest {
  name: string;
  format: 'openai' | 'claude' | 'gemini';
  upstream?: string;
  api_key?: string;
  base_url?: string | null;
  proxy_url?: string | null;
  prefix?: string | null;
  models?: string[];
  excluded_models?: string[];
  headers?: Record<string, string>;
  disabled?: boolean;
  wire_api?: string;
  weight?: number;
  region?: string | null;
}

export interface ProtocolEndpointEntry {
  id: string;
  family: 'open_ai' | 'claude' | 'gemini';
  method: string;
  path: string;
  description: string;
  scope: 'public' | 'provider_scoped';
  transport: 'http' | 'web_socket';
  operation: 'generate' | 'count_tokens' | 'list_models';
  stream_transport: 'none' | 'sse' | 'web_socket_events';
  state: {
    status: 'verified' | 'failed' | 'unknown' | 'unsupported';
    message?: string | null;
  };
  note?: string | null;
}

export interface ProtocolCoverageEntry {
  provider: string;
  format: 'openai' | 'claude' | 'gemini';
  upstream: string;
  upstream_protocol: string;
  wire_api: string;
  disabled: boolean;
  surface_id: string;
  surface_label: string;
  ingress_protocol: 'open_ai' | 'claude' | 'gemini';
  execution_mode?: 'native' | 'lossless_adapted' | 'lossy_adapted' | null;
  state: {
    status: 'verified' | 'failed' | 'unknown' | 'unsupported';
    message?: string | null;
  };
}

export interface ProtocolMatrixResponse {
  endpoints: ProtocolEndpointEntry[];
  coverage: ProtocolCoverageEntry[];
}
