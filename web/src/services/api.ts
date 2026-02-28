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

export const providersApi = {
  list: () => api.get<Provider[]>('/providers'),

  get: (id: string) => api.get<Provider>(`/providers/${id}`),

  create: (data: ProviderCreateRequest) =>
    api.post<Provider>('/providers', data),

  update: (id: string, data: ProviderUpdateRequest) =>
    api.patch<Provider>(`/providers/${id}`, data),

  delete: (id: string) => api.delete(`/providers/${id}`),
};

// ── Auth Keys ──

export const authKeysApi = {
  list: () => api.get<AuthKey[]>('/auth-keys'),

  create: (data: AuthKeyCreateRequest) =>
    api.post<AuthKeyCreateResponse>('/auth-keys', data),

  delete: (id: string) => api.delete(`/auth-keys/${id}`),
};

// ── Routing ──

export const routingApi = {
  get: () => api.get<RoutingConfig>('/routing'),

  update: (data: RoutingUpdateRequest) =>
    api.patch<RoutingConfig>('/routing', data),
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
    return api.get<PaginatedResponse<RequestLog>>(`/logs?${params.toString()}`);
  },

  stats: () => api.get('/logs/stats'),
};

// ── System ──

export const systemApi = {
  health: () => api.get<SystemHealth>('/system/health'),

  logs: (page: number = 1, level?: string) => {
    const params = new URLSearchParams();
    params.set('page', String(page));
    if (level) params.set('level', level);
    return api.get<PaginatedResponse<SystemLog>>(`/system/logs?${params.toString()}`);
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
