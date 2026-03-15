// ── Auth ──

export interface LoginRequest {
  username: string;
  password: string;
}

export interface LoginResponse {
  authenticated: boolean;
  username: string;
  expires_in: number;
}

export interface SessionResponse {
  authenticated: boolean;
  username: string;
}

// ── Provider ──

export interface Provider {
  name: string;
  format: FormatType;
  upstream: UpstreamType;
  base_url: string | null;
  proxy_url: string | null;
  api_key_masked: string;
  api_key?: string;
  prefix: string | null;
  disabled: boolean;
  models: ModelMapping[];
  excluded_models: string[];
  headers?: Record<string, string>;
  wire_api: 'chat' | 'responses';
  weight: number;
  region: string | null;
  upstream_presentation?: UpstreamPresentation;
  auth_profiles?: ProviderAuthProfile[];
}

export type AuthMode =
  | 'api-key'
  | 'bearer-token'
  | 'codex-oauth'
  | 'anthropic-claude-subscription';

export interface ProviderAuthProfile {
  id: string;
  qualified_name: string;
  mode: AuthMode;
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
  headers?: Record<string, string>;
  disabled: boolean;
  weight: number;
  region?: string | null;
  prefix?: string | null;
  upstream_presentation?: UpstreamPresentation;
}

export interface ModelMapping {
  id: string;
  alias: string | null;
}

export type FormatType = 'openai' | 'claude' | 'gemini';
export type UpstreamType = 'openai' | 'codex' | 'claude' | 'gemini';

export type ProfileKind = 'native' | 'claude-code' | 'gemini-cli' | 'codex-cli';
export type ActivationMode = 'always' | 'auto';

export interface UpstreamPresentation {
  profile: ProfileKind;
  mode: ActivationMode;
  'strict-mode': boolean;
  'sensitive-words': string[];
  'cache-user-id': boolean;
  'custom-headers': Record<string, string>;
}

export interface PresentationPreviewResponse {
  profile: string;
  activated: boolean;
  effective_headers: Record<string, string>;
  body_mutations: { kind: string; applied: boolean; reason?: string }[];
  protected_headers_blocked: string[];
  effective_body: unknown;
}

export interface ProviderCreateRequest {
  name: string;
  format: FormatType;
  upstream?: UpstreamType;
  base_url?: string;
  proxy_url?: string;
  api_key?: string;
  prefix?: string;
  disabled: boolean;
  models: string[];
  excluded_models?: string[];
  headers?: Record<string, string>;
  wire_api?: string;
  weight?: number;
  region?: string;
  upstream_presentation?: UpstreamPresentation;
  auth_profiles?: ProviderAuthProfile[];
}

export interface ProviderUpdateRequest {
  upstream?: UpstreamType | null;
  base_url?: string | null;
  proxy_url?: string | null;
  api_key?: string;
  prefix?: string | null;
  disabled?: boolean;
  models?: string[];
  excluded_models?: string[];
  headers?: Record<string, string>;
  wire_api?: string | null;
  weight?: number;
  region?: string | null;
  upstream_presentation?: UpstreamPresentation | null;
  auth_profiles?: ProviderAuthProfile[];
}

export interface AuthProfile {
  provider: string;
  format: FormatType;
  id: string;
  qualified_name: string;
  mode: AuthMode;
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
  headers?: Record<string, string>;
  disabled: boolean;
  weight: number;
  region?: string | null;
  prefix?: string | null;
  upstream_presentation?: UpstreamPresentation;
}

export interface AuthProfileUpsertRequest {
  provider?: string;
  id?: string;
  mode: AuthMode;
  header?: string;
  secret?: string | null;
  headers?: Record<string, string>;
  disabled?: boolean;
  weight?: number;
  region?: string | null;
  prefix?: string | null;
  upstream_presentation?: UpstreamPresentation;
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

export interface CodexOauthCompleteResponse {
  profile: AuthProfile;
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
  profile?: AuthProfile;
}

export interface ConnectAuthProfileRequest {
  secret: string;
}

// ── Provider Capabilities ──

export interface ProviderCapabilities {
  supports_stream: boolean;
  supports_tools: boolean;
  supports_parallel_tools: boolean;
  supports_json_schema: boolean;
  supports_reasoning: boolean;
  supports_images: boolean;
  supports_count_tokens: boolean;
}

export type ProbeStatus = 'verified' | 'failed' | 'unknown' | 'unsupported';

export interface CapabilityProbeState {
  status: ProbeStatus;
  message?: string | null;
}

export interface CapabilityProbeStates {
  text: CapabilityProbeState;
  stream: CapabilityProbeState;
  tools: CapabilityProbeState;
  images: CapabilityProbeState;
  json_schema: CapabilityProbeState;
  reasoning: CapabilityProbeState;
  count_tokens: CapabilityProbeState;
}

export interface ProviderCapabilityEntry {
  name: string;
  upstream_protocol: string;
  models: string[];
  capabilities: ProviderCapabilities;
  probe: CapabilityProbeStates;
  disabled: boolean;
}

export interface ProviderProbeCheck {
  capability: string;
  status: ProbeStatus;
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

// ── Auth Keys ──

export interface AuthKey {
  id: number;
  key_masked: string;
  name: string | null;
  tenant_id: string | null;
  allowed_models: string[];
  allowed_credentials: string[];
  rate_limit: KeyRateLimitConfig | null;
  budget: BudgetConfig | null;
  expires_at: string | null;
  metadata: Record<string, string>;
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

export interface AuthKeyCreateResponse {
  key: string;
  message: string;
}

// ── Routing ──

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
  aliases?: { from: string; to: string }[];
  rewrites?: { pattern: string; to: string }[];
  fallbacks?: { pattern: string; to: string[] }[];
  'provider-pins'?: { pattern: string; providers: string[] }[];
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

// ── Route Introspection (canonical model for preview & explain) ──

export interface RouteIntrospectionRequest {
  model: string;
  endpoint?: string;
  source_format?: string;
  tenant_id?: string;
  api_key_id?: string;
  region?: string;
  stream?: boolean;
  headers?: Record<string, string>;
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

/** Rejection reason is a serde enum: string for unit variants, object for struct variants. */
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

/** Format a RejectReason for display. */
export function formatRejectReason(reason: RejectReason): string {
  if (typeof reason === 'string') {
    return reason.replace(/_/g, ' ');
  }
  if (typeof reason === 'object' && 'missing_capability' in reason) {
    return `missing capability: ${reason.missing_capability.capabilities.join(', ')}`;
  }
  return String(reason);
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

// ── Metrics (real-time WebSocket snapshot) ──

export interface MetricsSnapshot {
  total_requests: number;
  total_errors: number;
  total_tokens: number;
  active_providers: number;
  requests_per_minute: number;
  avg_latency_ms: number;
  error_rate: number;
  uptime_seconds: number;
  [key: string]: unknown;
}

// ── Request Logs ──

export interface TokenUsage {
  input_tokens: number;
  output_tokens: number;
  cache_read_tokens: number;
  cache_creation_tokens: number;
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

export interface RequestLogFilter {
  // Exact match
  request_id?: string;
  tenant_id?: string;
  api_key_id?: string;
  // Filter
  provider?: string;
  model?: string;
  status?: string;
  error_type?: string;
  stream?: boolean;
  // Range (epoch ms)
  from?: number;
  to?: number;
  latency_min?: number;
  latency_max?: number;
  // Keyword search
  keyword?: string;
  // Sort
  sort_by?: 'timestamp' | 'latency' | 'cost';
  sort_order?: 'asc' | 'desc';
}

// ── Log Stats (from /logs/stats) ──

export interface LogStats {
  total_entries: number;
  error_count: number;
  avg_latency_ms: number;
  p50_latency_ms: number;
  p95_latency_ms: number;
  p99_latency_ms: number;
  total_cost: number;
  total_tokens: number;
  time_series: TimeSeriesBucket[];
  top_models: ModelStats[];
  top_errors: ErrorStats[];
  provider_distribution: ProviderDistribution[];
  status_distribution: StatusDistribution;
}

export interface TimeSeriesBucket {
  timestamp: string;
  requests: number;
  errors: number;
  avg_latency_ms: number;
  tokens: number;
  cost: number;
}

export interface ModelStats {
  model: string;
  requests: number;
  avg_latency_ms: number;
  total_tokens: number;
  total_cost: number;
}

export interface ErrorStats {
  error_type: string;
  count: number;
  last_seen: string;
}

export interface ProviderDistribution {
  provider: string;
  requests: number;
  percentage: number;
}

export interface StatusDistribution {
  success: number;
  client_error: number;
  server_error: number;
}

export interface FilterOptions {
  providers: string[];
  models: string[];
  error_types: string[];
  tenant_ids: string[];
}

export type TimeRange = '5m' | '15m' | '1h' | '6h' | '24h';

export interface PaginatedResponse<T> {
  data: T[];
  page: number;
  page_size: number;
  total: number;
  total_pages: number;
}

// ── System ──

export interface SystemHealth {
  status: 'healthy' | 'degraded' | 'unhealthy';
  uptime_seconds: number;
  version: string;
  host: string;
  port: number;
  tls_enabled: boolean;
  providers: ProviderHealth[];
  metrics?: {
    total_requests: number;
    total_errors: number;
    error_rate: number;
    avg_latency_ms: number;
    rpm: number;
    total_tokens: number;
    total_cost_usd: number;
    cache_hits: number;
    cache_misses: number;
  };
}

export interface ProviderHealth {
  name: string;
  status: 'healthy' | 'degraded' | 'unhealthy' | 'unconfigured';
  active_keys: number;
  total_keys: number;
}

export interface SystemLog {
  timestamp: string;
  level: 'TRACE' | 'DEBUG' | 'INFO' | 'WARN' | 'ERROR';
  target: string;
  message: string;
}

// ── Tenants ──

export interface TenantSummary {
  id: string;
  requests: number;
  tokens: number;
  cost_usd: number;
}

export interface TenantMetricsResponse {
  tenant_id: string;
  metrics: {
    requests: number;
    tokens: number;
    cost_usd: number;
  } | null;
}

// ── WebSocket ──

export interface WsMessage {
  type: 'metrics' | 'request_log';
  data: MetricsSnapshot | RequestLog;
}

// ── Config ──

export interface ConfigProviderSummary {
  name: string;
  format: string;
  disabled: boolean;
  models_count: number;
  region: string | null;
  wire_api: string;
}

export interface ConfigSnapshot {
  listen: {
    host: string;
    port: number;
    tls_enabled: boolean;
    body_limit_mb: number;
  };
  providers: {
    total: number;
    items: ConfigProviderSummary[];
  };
  routing: RoutingConfig;
  auth_keys: {
    total: number;
  };
  dashboard: {
    enabled: boolean;
    username: string;
    jwt_ttl_secs: number;
  };
  rate_limit: Record<string, unknown>;
  cache: {
    enabled: boolean;
    max_entries: number;
    ttl_secs: number;
  };
  cost: {
    custom_prices_count: number;
  };
  retry: Record<string, unknown>;
  streaming: Record<string, unknown>;
  timeouts: {
    connect_timeout: number;
    request_timeout: number;
  };
  log_store: {
    capacity: number;
  };
  config_version: string;
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
