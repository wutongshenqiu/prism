// ── Auth ──

export interface LoginRequest {
  username: string;
  password: string;
}

export interface LoginResponse {
  token: string;
  expires_in: number;
}

export interface AuthState {
  token: string | null;
  isAuthenticated: boolean;
  login: (username: string, password: string) => Promise<void>;
  logout: () => void;
  refreshToken: () => Promise<void>;
}

// ── Provider ──

export interface Provider {
  id: string;
  name: string;
  provider_type: ProviderType;
  base_url: string;
  api_key?: string;
  enabled: boolean;
  models: string[];
  created_at: string;
  updated_at: string;
}

export type ProviderType = 'openai' | 'claude' | 'gemini' | 'openai_compat';

export interface ProviderCreateRequest {
  name: string;
  provider_type: ProviderType;
  base_url: string;
  api_key: string;
  enabled: boolean;
  models: string[];
}

export interface ProviderUpdateRequest {
  name?: string;
  base_url?: string;
  api_key?: string;
  enabled?: boolean;
  models?: string[];
}

// ── Auth Keys ──

export interface AuthKey {
  id: string;
  name: string;
  key_prefix: string;
  created_at: string;
  last_used_at: string | null;
  expires_at: string | null;
}

export interface AuthKeyCreateRequest {
  name: string;
  expires_in_days?: number;
}

export interface AuthKeyCreateResponse {
  id: string;
  name: string;
  key: string;
  expires_at: string | null;
}

// ── Routing ──

export type RoutingStrategy = 'round_robin' | 'random' | 'least_latency' | 'failover';

export interface RoutingConfig {
  strategy: RoutingStrategy;
  fallback_enabled: boolean;
  retry_count: number;
  timeout_ms: number;
}

export interface RoutingUpdateRequest {
  strategy?: RoutingStrategy;
  fallback_enabled?: boolean;
  retry_count?: number;
  timeout_ms?: number;
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

export interface MetricsState {
  snapshot: MetricsSnapshot | null;
  timeSeries: MetricsTimeSeries[];
  providerDistribution: ProviderDistribution[];
  latencyBuckets: LatencyBucket[];
  setSnapshot: (snapshot: MetricsSnapshot) => void;
  addTimeSeriesPoint: (point: MetricsTimeSeries) => void;
  setProviderDistribution: (data: ProviderDistribution[]) => void;
  setLatencyBuckets: (data: LatencyBucket[]) => void;
  fetchStats: () => Promise<void>;
}

// ── Request Logs ──

export interface RequestLog {
  id: string;
  timestamp: string;
  method: string;
  path: string;
  provider: string;
  model: string;
  status: number;
  latency_ms: number;
  input_tokens: number;
  output_tokens: number;
  error?: string;
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

export interface LogsState {
  logs: RequestLog[];
  page: number;
  pageSize: number;
  total: number;
  totalPages: number;
  filters: RequestLogFilter;
  setFilters: (filters: RequestLogFilter) => void;
  setPage: (page: number) => void;
  fetchLogs: () => Promise<void>;
  addLog: (log: RequestLog) => void;
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
