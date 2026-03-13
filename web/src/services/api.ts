import axios from 'axios';
import type {
  LoginResponse,
  Provider,
  ProviderCreateRequest,
  ProviderUpdateRequest,
  AuthKey,
  AuthKeyCreateRequest,
  AuthKeyCreateResponse,
  AuthKeyUpdateRequest,
  RoutingConfig,
  RoutingUpdateRequest,
  RequestLog,
  PaginatedResponse,
  RequestLogFilter,
  LogStats,
  FilterOptions,
  SystemHealth,
  SystemLog,
  ConfigValidateResponse,
  TenantSummary,
  TenantMetricsResponse,
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
        localStorage.setItem('auth_token', newToken);
        originalRequest.headers.Authorization = `Bearer ${newToken}`;

        return api(originalRequest);
      } catch {
        localStorage.removeItem('auth_token');
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

// ── Providers ──

export const providersApi = {
  list: () =>
    api.get('/providers').then((res) => {
      const raw = res.data.providers || res.data;
      const data = Array.isArray(raw) ? raw as Provider[] : [];
      return { ...res, data };
    }) as Promise<{ data: Provider[] } & Record<string, unknown>>,

  get: (id: string) => api.get<Provider>(`/providers/${id}`),

  create: (data: ProviderCreateRequest) =>
    api.post<Provider>('/providers', data),

  update: (id: string, data: ProviderUpdateRequest) =>
    api.patch<Provider>(`/providers/${id}`, data),

  delete: (id: string) => api.delete(`/providers/${id}`),

  fetchModels: (data: { provider_type: string; api_key: string; base_url?: string }) =>
    api.post<{ models: string[] }>('/providers/fetch-models', data)
      .then((res) => res.data.models),

  healthCheck: (id: string) =>
    api.post<{ status: string; latency_ms?: number; message?: string }>(`/providers/${id}/health`)
      .then((res) => res.data),
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
};

// ── Routing ──

export const routingApi = {
  get: () =>
    api.get('/routing').then((res) => {
      const raw = res.data;
      return {
        ...res,
        data: {
          strategy: raw.strategy ?? 'round-robin',
          fallback_enabled: raw.fallback_enabled ?? false,
          request_retry: raw.request_retry ?? 3,
          max_retry_interval: raw.max_retry_interval ?? 30,
          model_strategies: raw.model_strategies ?? {},
          model_fallbacks: raw.model_fallbacks ?? {},
        } as RoutingConfig,
      };
    }) as Promise<{ data: RoutingConfig } & Record<string, unknown>>,

  update: (data: RoutingUpdateRequest) =>
    api.patch('/routing', data),
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
  current: () => api.get('/config/current'),

  raw: () => api.get<{ content: string; path: string }>('/config/raw'),

  validate: (yaml: string) =>
    api.post<ConfigValidateResponse>('/config/validate', { yaml }),

  reload: () => api.post('/config/reload'),

  apply: (yaml: string) =>
    api.put<{ message: string }>('/config/apply', { yaml }),
};

export default api;
