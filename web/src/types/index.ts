// ── Auth ──

export interface LoginRequest {
  username: string;
  password: string;
}

export interface LoginResponse {
  token: string;
  expires_in: number;
}

// ── Provider ──

export interface Provider {
  id: string;
  name: string | null;
  provider_type: ProviderType;
  base_url: string | null;
  api_key_masked: string;
  api_key?: string;
  enabled: boolean;
  disabled: boolean;
  models: string[];
  models_count: number;
  headers?: Record<string, string>;
  created_at?: string;
  updated_at?: string;
}

export type ProviderType = 'openai' | 'claude' | 'gemini' | 'openai_compat';

export interface ProviderCreateRequest {
  name: string;
  provider_type: ProviderType;
  base_url: string;
  api_key: string;
  enabled: boolean;
  models: string[];
  headers?: Record<string, string>;
}

export interface ProviderUpdateRequest {
  name?: string;
  base_url?: string;
  api_key?: string;
  enabled?: boolean;
  models?: string[];
  headers?: Record<string, string>;
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

export type RoutingStrategy = 'round-robin' | 'fill-first' | 'latency-aware' | 'geo-aware';

export interface RoutingConfig {
  strategy: RoutingStrategy;
  fallback_enabled: boolean;
  request_retry: number;
  max_retry_interval: number;
  model_strategies: Record<string, RoutingStrategy>;
  model_fallbacks: Record<string, string[]>;
}

export interface RoutingUpdateRequest {
  strategy?: RoutingStrategy;
  fallback_enabled?: boolean;
  request_retry?: number;
  max_retry_interval?: number;
  model_strategies?: Record<string, RoutingStrategy>;
  model_fallbacks?: Record<string, string[]>;
}

// ── Metrics ──

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

export interface MetricsTimeSeries {
  timestamp: string;
  requests: number;
  errors: number;
  tokens: number;
  latency_ms: number;
}

export interface ProviderDistribution {
  provider: string;
  requests: number;
  percentage: number;
}

export interface LatencyBucket {
  range: string;
  count: number;
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
  provider?: string;
  model?: string;
  status?: string;
  date_from?: string;
  date_to?: string;
}

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
  providers: ProviderHealth[];
  memory_usage_mb: number;
  cpu_usage_percent: number;
}

export interface ProviderHealth {
  name: string;
  status: 'up' | 'down' | 'degraded';
  latency_ms: number;
  last_check: string;
}

export interface SystemLog {
  timestamp: string;
  level: 'TRACE' | 'DEBUG' | 'INFO' | 'WARN' | 'ERROR';
  target: string;
  message: string;
}

// ── WebSocket ──

export interface WsMessage {
  type: 'metrics' | 'request_log';
  data: MetricsSnapshot | RequestLog;
}

// ── Config ──

export interface ConfigValidateResponse {
  valid: boolean;
  errors: string[];
}
