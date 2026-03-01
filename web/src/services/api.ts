import axios from 'axios';
import type {
  LoginResponse,
  Provider,
  ProviderCreateRequest,
  ProviderUpdateRequest,
  AuthKey,
  AuthKeyCreateRequest,
  AuthKeyCreateResponse,
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

    if (error.response?.status === 401 && !originalRequest._retry) {
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
            models: Array.isArray(item.models) ? item.models : [],
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
      enabled: (res.data as Record<string, unknown>).disabled !== undefined
        ? !(res.data as Record<string, unknown>).disabled
        : true,
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
};

// ── Auth Keys ──

export const authKeysApi = {
  list: () =>
    api.get('/auth-keys').then((res) => {
      const raw = res.data.auth_keys || res.data;
      const data = Array.isArray(raw)
        ? raw.map((item: Record<string, unknown>) => ({
            id: String(item.id ?? ''),
            name: (item.name as string) || '',
            key_prefix: (item.key_prefix as string) || (item.key_masked as string) || '',
            created_at: (item.created_at as string) || '',
            last_used_at: (item.last_used_at as string) || null,
            expires_at: (item.expires_at as string) || null,
          }))
        : [];
      return { ...res, data };
    }) as Promise<{ data: AuthKey[] } & Record<string, unknown>>,

  create: (data: AuthKeyCreateRequest) =>
    api.post<AuthKeyCreateResponse>('/auth-keys', data),

  delete: (id: string) => api.delete(`/auth-keys/${id}`),
};

// ── Routing ──

// Map backend strategy names (kebab-case) to frontend (snake_case)
const strategyMap: Record<string, string> = {
  'round-robin': 'round_robin',
  'fill-first': 'failover',
  'random': 'random',
  'least-latency': 'least_latency',
};
const reverseStrategyMap: Record<string, string> = Object.fromEntries(
  Object.entries(strategyMap).map(([k, v]) => [v, k])
);

export const routingApi = {
  get: () =>
    api.get('/routing').then((res) => {
      const raw = res.data;
      return {
        ...res,
        data: {
          strategy: (strategyMap[raw.strategy] || raw.strategy) as RoutingConfig['strategy'],
          fallback_enabled: raw.fallback_enabled ?? false,
          retry_count: raw.retry_count ?? raw.request_retry ?? 3,
          timeout_ms: raw.timeout_ms ?? (raw.max_retry_interval ? raw.max_retry_interval * 1000 : 30000),
        },
      };
    }) as Promise<{ data: RoutingConfig } & Record<string, unknown>>,

  update: (data: RoutingUpdateRequest) => {
    // Convert frontend fields back to backend format
    const payload: Record<string, unknown> = {};
    if (data.strategy) payload.strategy = reverseStrategyMap[data.strategy] || data.strategy;
    if (data.fallback_enabled !== undefined) payload.fallback_enabled = data.fallback_enabled;
    if (data.retry_count !== undefined) payload.request_retry = data.retry_count;
    if (data.timeout_ms !== undefined) payload.max_retry_interval = Math.round(data.timeout_ms / 1000);
    return api.patch<RoutingConfig>('/routing', payload);
  },
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
            input_tokens: item.input_tokens ?? 0,
            output_tokens: item.output_tokens ?? 0,
            provider: item.provider || '-',
            model: item.model || '-',
            timestamp: typeof item.timestamp === 'number'
              ? new Date(item.timestamp).toISOString()
              : item.timestamp,
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
