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
  SystemHealth,
  SystemLog,
  ConfigValidateResponse,
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

// Map provider_type between frontend (underscore) and backend (hyphen)
const providerTypeToBackend = (type: string): string =>
  type === 'openai_compat' ? 'openai-compat' : type;
const providerTypeToFrontend = (type: string): string =>
  type === 'openai-compat' ? 'openai_compat' : type;

export const providersApi = {
  list: () =>
    api.get('/providers').then((res) => {
      // Backend returns {providers: [...]}; normalize to Provider[]
      const raw = res.data.providers || res.data;
      const data = Array.isArray(raw)
        ? raw.map((item: Record<string, unknown>) => ({
            ...item,
            provider_type: providerTypeToFrontend((item.provider_type as string) || ''),
            name: (item.name as string) || '',
            base_url: (item.base_url as string) || '',
            enabled: item.disabled !== undefined ? !(item.disabled as boolean) : true,
            models: Array.isArray(item.models)
              ? item.models.map((m: unknown) => typeof m === 'string' ? m : (m as Record<string, unknown>)?.id ?? m)
              : [],
            headers: (item.headers as Record<string, string>) || {},
            models_count: item.models_count,
          }))
        : [];
      return { ...res, data };
    }) as Promise<{ data: Provider[] } & Record<string, unknown>>,

  get: (id: string) => api.get<Provider>(`/providers/${id}`).then((res) => ({
    ...res,
    data: {
      ...res.data,
      provider_type: providerTypeToFrontend(res.data.provider_type),
      enabled: res.data.disabled !== undefined ? !res.data.disabled : true,
      models: Array.isArray(res.data.models)
        ? res.data.models.map((m: unknown) => typeof m === 'string' ? m : (m as Record<string, unknown>)?.id ?? m)
        : [],
      headers: res.data.headers || {},
    },
  })),

  create: (data: ProviderCreateRequest) =>
    api.post<Provider>('/providers', {
      ...data,
      provider_type: providerTypeToBackend(data.provider_type),
      disabled: !data.enabled,
    }),

  update: (id: string, data: ProviderUpdateRequest) =>
    api.patch<Provider>(`/providers/${id}`, {
      ...data,
      disabled: data.enabled !== undefined ? !data.enabled : undefined,
      enabled: undefined,
    }),

  delete: (id: string) => api.delete(`/providers/${id}`),

  fetchModels: (data: { provider_type: string; api_key: string; base_url?: string }) =>
    api.post<{ models: string[] }>('/providers/fetch-models', {
      ...data,
      provider_type: data.provider_type === 'openai_compat' ? 'openai-compat' : data.provider_type,
    }).then((res) => res.data.models),

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

export const logsApi = {
  list: (
    page: number = 1,
    pageSize: number = 50,
    filters?: RequestLogFilter
  ) => {
    const params = new URLSearchParams();
    params.set('page', String(page));
    params.set('page_size', String(pageSize));
    if (filters?.provider) params.set('provider', filters.provider);
    if (filters?.model) params.set('model', filters.model);
    if (filters?.status) params.set('status', filters.status);
    if (filters?.date_from) params.set('date_from', filters.date_from);
    if (filters?.date_to) params.set('date_to', filters.date_to);
    return api.get(`/logs?${params.toString()}`).then((res) => {
      // Backend returns {items, page, page_size, total}; normalize to PaginatedResponse
      const raw = res.data;
      const items = raw.items || raw.data || [];
      const total = raw.total || 0;
      const pageSize = raw.page_size || 50;
      return {
        ...res,
        data: {
          data: items.map((item: Record<string, unknown>) => ({
            ...item,
            id: item.request_id || item.id || '',
          })),
          total,
          total_pages: Math.ceil(total / pageSize),
          page: raw.page || 1,
          page_size: pageSize,
        },
      };
    }) as Promise<{ data: PaginatedResponse<RequestLog> } & Record<string, unknown>>;
  },

  stats: () => api.get('/logs/stats'),
};

// ── System ──

export const systemApi = {
  health: () =>
    api.get('/system/health').then((res) => {
      // Normalize backend format to frontend SystemHealth
      const raw = res.data;
      const providerObj = raw.providers || {};
      const providersList = Array.isArray(providerObj)
        ? providerObj
        : Object.entries(providerObj).map(([name, count]) => ({
            name,
            status: (count as number) > 0 ? 'healthy' : 'unhealthy',
            latency_ms: 0,
            last_error: null,
          }));
      return {
        ...res,
        data: {
          status: raw.status || 'healthy',
          uptime_seconds: raw.uptime_secs ?? raw.uptime_seconds ?? 0,
          version: raw.version || '0.0.0',
          providers: providersList,
          memory_usage_mb: raw.memory_usage_mb ?? 0,
          cpu_usage_percent: raw.cpu_usage_percent ?? 0,
        },
      };
    }) as Promise<{ data: SystemHealth } & Record<string, unknown>>,

  logs: (page: number = 1, level?: string) => {
    const params = new URLSearchParams();
    params.set('page', String(page));
    if (level) params.set('level', level);
    return api.get(`/system/logs?${params.toString()}`).then((res) => {
      const raw = res.data;
      const logs = raw.logs || raw.data || [];
      const total = raw.total || 0;
      return {
        ...res,
        data: {
          data: logs,
          total,
          total_pages: Math.ceil(total / 50),
          page: raw.page || 1,
          page_size: 50,
        },
      };
    }) as Promise<{ data: PaginatedResponse<SystemLog> } & Record<string, unknown>>;
  },
};

// ── Config ──

export const configApi = {
  current: () => api.get('/config/current'),

  validate: (config: string) =>
    api.post<ConfigValidateResponse>('/config/validate', { config }),

  reload: () => api.post('/config/reload'),
};

export default api;
