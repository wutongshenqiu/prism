import axios from 'axios';
import type {
  LoginResponse,
  Provider,
  ProviderCreateRequest,
  ProviderUpdateRequest,
  PresentationPreviewResponse,
  ProviderCapabilityEntry,
  AuthKey,
  AuthKeyCreateRequest,
  AuthKeyCreateResponse,
  AuthKeyUpdateRequest,
  RoutingConfig,
  RoutingUpdateRequest,
  RouteIntrospectionRequest,
  RouteExplanation,
  RequestLog,
  PaginatedResponse,
  RequestLogFilter,
  LogStats,
  FilterOptions,
  SystemHealth,
  SystemLog,
  ConfigValidateResponse,
  ConfigSnapshot,
  RawConfigResponse,
  TenantSummary,
  TenantMetricsResponse,
  AuthProfile,
  AuthProfileUpsertRequest,
  CodexOauthStartRequest,
  CodexOauthStartResponse,
  CodexOauthCompleteResponse,
  ConnectAuthProfileRequest,
  ProviderAuthProfile,
} from '../types';

const api = axios.create({
  baseURL: '/api/dashboard',
  headers: {
    'Content-Type': 'application/json',
  },
});

// Request interceptor: attach JWT token
api.interceptors.request.use((config) => {
  const token = localStorage.getItem('auth_token');
  if (token) {
    config.headers.Authorization = `Bearer ${token}`;
  }
  return config;
});

// Token state bridge: centralizes token read/write so both the interceptor
// and the Zustand auth store stay in sync without circular imports.
// The auth store registers itself on init via `setTokenSetter`.
let _tokenSetter: ((token: string | null) => void) | null = null;

export function setTokenSetter(setter: (token: string | null) => void): void {
  _tokenSetter = setter;
}

function setToken(token: string): void {
  localStorage.setItem('auth_token', token);
  _tokenSetter?.(token);
}

function clearToken(): void {
  localStorage.removeItem('auth_token');
  _tokenSetter?.(null);
}

// Response interceptor: handle 401 and token refresh
api.interceptors.response.use(
  (response) => response,
  async (error) => {
    const originalRequest = error.config;

    // Skip token refresh for login/refresh endpoints — let the caller handle the error
    const isAuthEndpoint = originalRequest.url?.includes('/auth/login') || originalRequest.url?.includes('/auth/refresh');
    if (error.response?.status === 401 && !originalRequest._retry && !isAuthEndpoint) {
      originalRequest._retry = true;

      try {
        const token = localStorage.getItem('auth_token');
        if (!token) throw new Error('No token');

        const response = await axios.post<LoginResponse>(
          '/api/dashboard/auth/refresh',
          null,
          { headers: { Authorization: `Bearer ${token}` } }
        );

        const newToken = response.data.token;
        setToken(newToken);
        originalRequest.headers.Authorization = `Bearer ${newToken}`;

        return api(originalRequest);
      } catch {
        clearToken();
        window.location.href = '/login';
        return Promise.reject(error);
      }
    }

    return Promise.reject(error);
  }
);

// ── Auth ──

export const authApi = {
  login: (username: string, password: string) =>
    api.post<LoginResponse>('/auth/login', { username, password }),

  refresh: () => api.post<LoginResponse>('/auth/refresh'),
};

function asRecord(value: unknown): Record<string, unknown> {
  return typeof value === 'object' && value !== null ? value as Record<string, unknown> : {};
}

function asStringMap(value: unknown): Record<string, string> {
  const record = asRecord(value);
  return Object.fromEntries(
    Object.entries(record)
      .filter(([, entry]) => typeof entry === 'string')
      .map(([key, entry]) => [key, entry as string]),
  );
}

function normalizeAuthProfile(raw: unknown): AuthProfile {
  const record = asRecord(raw);
  return {
    provider: String(record.provider ?? ''),
    format: (record.format as AuthProfile['format']) ?? 'openai',
    id: String(record.id ?? ''),
    qualified_name: String(record.qualified_name ?? ''),
    mode: (record.mode as AuthProfile['mode']) ?? 'api-key',
    header: String(record.header ?? 'auto'),
    connected: Boolean(record.connected),
    secret_masked: (record.secret_masked as string | null) ?? null,
    access_token_masked: (record.access_token_masked as string | null) ?? null,
    refresh_token_present: Boolean(record.refresh_token_present),
    id_token_present: Boolean(record.id_token_present),
    expires_at: (record.expires_at as string | null) ?? null,
    account_id: (record.account_id as string | null) ?? null,
    email: (record.email as string | null) ?? null,
    last_refresh: (record.last_refresh as string | null) ?? null,
    headers: asStringMap(record.headers),
    disabled: Boolean(record.disabled),
    weight: Number(record.weight ?? 1),
    region: (record.region as string | null) ?? null,
    prefix: (record.prefix as string | null) ?? null,
    upstream_presentation: (record.upstream_presentation as AuthProfile['upstream_presentation']) ?? undefined,
  };
}

function normalizeProvider(raw: unknown): Provider {
  const record = asRecord(raw);
  const models = Array.isArray(record.models) ? record.models : [];
  const authProfiles = Array.isArray(record.auth_profiles) ? record.auth_profiles : [];
  return {
    name: String(record.name ?? ''),
    format: (record.format as Provider['format']) ?? 'openai',
    base_url: (record.base_url as string | null) ?? null,
    proxy_url: (record.proxy_url as string | null) ?? null,
    api_key_masked: String(record.api_key_masked ?? ''),
    api_key: (record.api_key as string | undefined) ?? undefined,
    prefix: (record.prefix as string | null) ?? null,
    disabled: Boolean(record.disabled),
    models: models.map((item) => {
      const model = asRecord(item);
      return {
        id: String(model.id ?? ''),
        alias: (model.alias as string | null) ?? null,
      };
    }),
    excluded_models: Array.isArray(record.excluded_models) ? record.excluded_models.map(String) : [],
    headers: asStringMap(record.headers),
    wire_api: (record.wire_api as Provider['wire_api']) ?? 'chat',
    weight: Number(record.weight ?? 1),
    region: (record.region as string | null) ?? null,
    upstream_presentation: (record.upstream_presentation as Provider['upstream_presentation']) ?? undefined,
    auth_profiles: authProfiles.map((item) => normalizeAuthProfile(item) as ProviderAuthProfile),
  };
}

// ── Providers ──

export const providersApi = {
  list: () =>
    api.get('/providers').then((res) => {
      const raw = res.data.providers || res.data;
      const data = Array.isArray(raw) ? raw.map(normalizeProvider) : [];
      return { ...res, data };
    }) as Promise<{ data: Provider[] } & Record<string, unknown>>,

  get: (name: string) =>
    api.get(`/providers/${encodeURIComponent(name)}`).then((res) => ({
      ...res,
      data: normalizeProvider(res.data),
    })) as Promise<{ data: Provider } & Record<string, unknown>>,

  create: (data: ProviderCreateRequest) =>
    api.post<Provider>('/providers', data),

  update: (name: string, data: ProviderUpdateRequest) =>
    api.patch<Provider>(`/providers/${encodeURIComponent(name)}`, data),

  delete: (name: string) => api.delete(`/providers/${encodeURIComponent(name)}`),

  fetchModels: (data: { format: string; api_key: string; base_url?: string }) =>
    api.post<{ models: string[] }>('/providers/fetch-models', data)
      .then((res) => res.data.models),

  healthCheck: (name: string) =>
    api.post<{ status: string; latency_ms?: number; message?: string }>(`/providers/${encodeURIComponent(name)}/health`)
      .then((res) => res.data),

  presentationPreview: (name: string, data: { model?: string; user_agent?: string; sample_body?: unknown }) =>
    api.post<PresentationPreviewResponse>(`/providers/${encodeURIComponent(name)}/presentation-preview`, data)
      .then((res) => res.data),

  capabilities: () =>
    api.get<{ providers: ProviderCapabilityEntry[] }>('/providers/capabilities')
      .then((res) => res.data.providers),
};

// ── Auth Profiles ──

export const authProfilesApi = {
  list: () =>
    api.get('/auth-profiles').then((res) => {
      const raw = res.data.profiles || [];
      const data = Array.isArray(raw) ? raw.map(normalizeAuthProfile) : [];
      return { ...res, data };
    }) as Promise<{ data: AuthProfile[] } & Record<string, unknown>>,

  create: (data: AuthProfileUpsertRequest & { provider: string; id: string }) =>
    api.post('/auth-profiles', data).then((res) => normalizeAuthProfile(asRecord(res.data).profile)),

  replace: (provider: string, profileId: string, data: AuthProfileUpsertRequest) =>
    api.put(`/auth-profiles/${encodeURIComponent(provider)}/${encodeURIComponent(profileId)}`, data)
      .then((res) => normalizeAuthProfile(asRecord(res.data).profile)),

  delete: (provider: string, profileId: string) =>
    api.delete(`/auth-profiles/${encodeURIComponent(provider)}/${encodeURIComponent(profileId)}`),

  startCodexOauth: (data: CodexOauthStartRequest) =>
    api.post<CodexOauthStartResponse>('/auth-profiles/codex/oauth/start', data)
      .then((res) => res.data),

  completeCodexOauth: (state: string, code: string) =>
    api.post<CodexOauthCompleteResponse>('/auth-profiles/codex/oauth/complete', { state, code })
      .then((res) => normalizeAuthProfile(asRecord(res.data).profile)),

  connect: (provider: string, profileId: string, data: ConnectAuthProfileRequest) =>
    api.post(`/auth-profiles/${encodeURIComponent(provider)}/${encodeURIComponent(profileId)}/connect`, data)
      .then((res) => normalizeAuthProfile(asRecord(res.data).profile)),

  refresh: (provider: string, profileId: string) =>
    api.post(`/auth-profiles/${encodeURIComponent(provider)}/${encodeURIComponent(profileId)}/refresh`)
      .then((res) => normalizeAuthProfile(asRecord(res.data).profile)),
};

// ── Auth Keys ──

export const authKeysApi = {
  list: () =>
    api.get('/auth-keys').then((res) => {
      const raw = res.data.auth_keys || res.data;
      const data = Array.isArray(raw)
        ? raw.map((item: Record<string, unknown>) => ({
            id: Number(item.id ?? 0),
            key_masked: (item.key_masked as string) || '',
            name: (item.name as string | null) ?? null,
            tenant_id: (item.tenant_id as string | null) ?? null,
            allowed_models: (item.allowed_models as string[]) || [],
            allowed_credentials: (item.allowed_credentials as string[]) || [],
            rate_limit: (item.rate_limit as AuthKey['rate_limit']) ?? null,
            budget: (item.budget as AuthKey['budget']) ?? null,
            expires_at: (item.expires_at as string | null) ?? null,
            metadata: (item.metadata as Record<string, string>) || {},
          }))
        : [];
      return { ...res, data };
    }) as Promise<{ data: AuthKey[] } & Record<string, unknown>>,

  create: (data: AuthKeyCreateRequest) =>
    api.post<AuthKeyCreateResponse>('/auth-keys', data),

  update: (id: number | string, data: AuthKeyUpdateRequest) =>
    api.patch(`/auth-keys/${id}`, data),

  delete: (id: number | string) => api.delete(`/auth-keys/${id}`),

  reveal: (id: number | string) =>
    api.post<{ key: string }>(`/auth-keys/${id}/reveal`).then((res) => res.data.key),
};

// ── Routing ──

export const routingApi = {
  get: () =>
    api.get('/routing').then((res) => {
      return {
        ...res,
        data: res.data as RoutingConfig,
      };
    }) as Promise<{ data: RoutingConfig } & Record<string, unknown>>,

  update: (data: RoutingUpdateRequest) =>
    api.patch('/routing', data),

  preview: (data: RouteIntrospectionRequest) =>
    api.post('/routing/preview', data).then((res) => ({
      ...res,
      data: res.data as RouteExplanation,
    })) as Promise<{ data: RouteExplanation } & Record<string, unknown>>,

  explain: (data: RouteIntrospectionRequest) =>
    api.post('/routing/explain', data).then((res) => ({
      ...res,
      data: res.data as RouteExplanation,
    })) as Promise<{ data: RouteExplanation } & Record<string, unknown>>,
};

// ── Logs ──

function setIfDefined(params: URLSearchParams, key: string, value: unknown) {
  if (value !== undefined && value !== null && value !== '') {
    params.set(key, String(value));
  }
}

export const logsApi = {
  list: (
    page: number = 1,
    pageSize: number = 50,
    filters?: RequestLogFilter
  ) => {
    const params = new URLSearchParams();
    params.set('page', String(page));
    params.set('page_size', String(pageSize));
    if (filters) {
      setIfDefined(params, 'provider', filters.provider);
      setIfDefined(params, 'model', filters.model);
      setIfDefined(params, 'status', filters.status);
      setIfDefined(params, 'error_type', filters.error_type);
      setIfDefined(params, 'request_id', filters.request_id);
      setIfDefined(params, 'tenant_id', filters.tenant_id);
      setIfDefined(params, 'api_key_id', filters.api_key_id);
      setIfDefined(params, 'keyword', filters.keyword);
      setIfDefined(params, 'from', filters.from);
      setIfDefined(params, 'to', filters.to);
      setIfDefined(params, 'latency_min', filters.latency_min);
      setIfDefined(params, 'latency_max', filters.latency_max);
      if (filters.stream !== undefined) params.set('stream', String(filters.stream));
      setIfDefined(params, 'sort_by', filters.sort_by);
      setIfDefined(params, 'sort_order', filters.sort_order);
    }
    return api.get(`/logs?${params.toString()}`).then((res) => {
      const raw = res.data;
      const items = raw.items || raw.data || [];
      const total = raw.total || 0;
      const ps = raw.page_size || 50;
      return {
        ...res,
        data: {
          data: items,
          total,
          total_pages: Math.ceil(total / ps),
          page: raw.page || 1,
          page_size: ps,
        },
      };
    }) as Promise<{ data: PaginatedResponse<RequestLog> } & Record<string, unknown>>;
  },

  getById: (id: string) =>
    api.get<RequestLog>(`/logs/${encodeURIComponent(id)}`),

  stats: (query?: { from?: number; to?: number; provider?: string; model?: string }) => {
    const params = new URLSearchParams();
    if (query) {
      setIfDefined(params, 'from', query.from);
      setIfDefined(params, 'to', query.to);
      setIfDefined(params, 'provider', query.provider);
      setIfDefined(params, 'model', query.model);
    }
    const qs = params.toString();
    return api.get<LogStats>(`/logs/stats${qs ? `?${qs}` : ''}`);
  },

  filters: () => api.get<FilterOptions>('/logs/filters'),
};

// ── System ──

export interface ProtocolMatrixEntry {
  provider: string;
  ingress_protocol: string;
  upstream_protocol: string;
  execution_mode: string;
  supports_generate: boolean;
  supports_stream: boolean;
  supports_count_tokens: boolean;
}

export const protocolsApi = {
  matrix: () =>
    api.get<{ entries: ProtocolMatrixEntry[] }>('/protocols/matrix').then((res) => res.data.entries),
};

export const systemApi = {
  health: () => api.get<SystemHealth>('/system/health'),

  logs: (page: number = 1, level?: string) => {
    const params = new URLSearchParams();
    params.set('page', String(page));
    if (level) params.set('level', level);
    return api.get(`/system/logs?${params.toString()}`).then((res) => {
      const raw = res.data;
      return {
        ...res,
        data: {
          data: raw.logs || [],
          total: raw.total || 0,
          total_pages: Math.ceil((raw.total || 0) / (raw.page_size || 50)),
          page: raw.page || 1,
          page_size: raw.page_size || 50,
        },
      };
    }) as Promise<{ data: PaginatedResponse<SystemLog> } & Record<string, unknown>>;
  },
};

// ── Tenants ──

export const tenantsApi = {
  list: () =>
    api.get('/tenants').then((res) => {
      const data = res.data.tenants || [];
      return { ...res, data };
    }) as Promise<{ data: TenantSummary[] } & Record<string, unknown>>,

  metrics: (id: string) =>
    api.get<TenantMetricsResponse>(`/tenants/${encodeURIComponent(id)}/metrics`),
};

// ── Config ──

export const configApi = {
  current: () => api.get<ConfigSnapshot>('/config/current'),

  raw: () => api.get<RawConfigResponse>('/config/raw'),

  validate: (yaml: string) =>
    api.post<ConfigValidateResponse>('/config/validate', { yaml }),

  reload: () => api.post('/config/reload'),

  apply: (yaml: string, configVersion?: string) =>
    api.put<{ message: string; config_version?: string }>('/config/apply', {
      yaml,
      ...(configVersion ? { config_version: configVersion } : {}),
    }),
};

export default api;
