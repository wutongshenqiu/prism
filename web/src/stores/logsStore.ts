import { create } from 'zustand';
import { logsApi } from '../services/api';
import type { RequestLog, RequestLogFilter, FilterOptions } from '../types';

interface LogsState {
  logs: RequestLog[];
  page: number;
  pageSize: number;
  total: number;
  totalPages: number;
  filters: RequestLogFilter;
  isLoading: boolean;
  error: string | null;

  // Filter options from backend
  filterOptions: FilterOptions | null;

  // Drawer state
  selectedLogId: string | null;
  selectedLog: RequestLog | null;
  isDrawerOpen: boolean;
  isLoadingDetail: boolean;
  detailError: string | null;

  // Live toggle
  isLive: boolean;

  setFilters: (filters: RequestLogFilter) => void;
  setPage: (page: number) => void;
  fetchLogs: () => Promise<void>;
  fetchFilterOptions: () => Promise<void>;
  addLog: (log: RequestLog) => void;
  openDrawer: (id: string) => Promise<void>;
  closeDrawer: () => void;
  toggleLive: () => void;
}

export const useLogsStore = create<LogsState>((set, get) => ({
  logs: [],
  page: 1,
  pageSize: 50,
  total: 0,
  totalPages: 0,
  filters: {},
  isLoading: false,
  error: null,

  filterOptions: null,

  selectedLogId: null,
  selectedLog: null,
  isDrawerOpen: false,
  isLoadingDetail: false,
  detailError: null,

  isLive: true,

  setFilters: (filters) => {
    set({ filters, page: 1 });
    get().fetchLogs();
  },

  setPage: (page) => {
    set({ page });
    get().fetchLogs();
  },

  fetchLogs: async () => {
    const { page, pageSize, filters } = get();
    set({ isLoading: true, error: null });
    try {
      const response = await logsApi.list(page, pageSize, filters);
      const data = response.data;
      set({
        logs: data.data,
        total: data.total,
        totalPages: data.total_pages,
        isLoading: false,
      });
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to fetch logs';
      set({ isLoading: false, error: message });
    }
  },

  fetchFilterOptions: async () => {
    try {
      const response = await logsApi.filters();
      set({ filterOptions: response.data });
    } catch (err) {
      console.error('Failed to fetch filter options:', err);
    }
  },

  addLog: (log) => {
    if (!get().isLive) return;
    const { filters } = get();
    // Filter incoming WebSocket logs against active filter set
    if (filters.provider && log.provider !== filters.provider) return;
    if (filters.model && log.model !== filters.model) return;
    if (filters.tenant_id && log.tenant_id !== filters.tenant_id) return;
    if (filters.api_key_id && log.api_key_id !== filters.api_key_id) return;
    if (filters.error_type && log.error_type !== filters.error_type) return;
    if (filters.stream !== undefined && log.stream !== filters.stream) return;
    if (filters.status) {
      const code = log.status;
      if (filters.status === '2xx' && (code < 200 || code >= 300)) return;
      if (filters.status === '4xx' && (code < 400 || code >= 500)) return;
      if (filters.status === '5xx' && (code < 500 || code >= 600)) return;
    }
    if (filters.latency_min && log.latency_ms < filters.latency_min) return;
    if (filters.latency_max && log.latency_ms > filters.latency_max) return;
    if (filters.keyword) {
      const kw = filters.keyword.toLowerCase();
      const haystack = `${log.requested_model || ''} ${log.model || ''} ${log.provider || ''} ${log.error || ''}`.toLowerCase();
      if (!haystack.includes(kw)) return;
    }
    set((state) => {
      const updated = [log, ...state.logs];
      if (updated.length > state.pageSize) {
        updated.pop();
      }
      return { logs: updated, total: state.total + 1 };
    });
  },

  openDrawer: async (id: string) => {
    // Use cached record from current page if available, avoiding a network round-trip
    const cached = get().logs.find((l) => l.request_id === id);
    if (cached) {
      set({ selectedLogId: id, isDrawerOpen: true, isLoadingDetail: false, selectedLog: cached });
      return;
    }
    set({ selectedLogId: id, isDrawerOpen: true, isLoadingDetail: true, selectedLog: null, detailError: null });
    try {
      const response = await logsApi.getById(id);
      set({ selectedLog: response.data, isLoadingDetail: false });
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to fetch log detail';
      set({ isLoadingDetail: false, detailError: message });
    }
  },

  closeDrawer: () => {
    set({ isDrawerOpen: false, selectedLogId: null, selectedLog: null, detailError: null });
  },

  toggleLive: () => {
    set((state) => ({ isLive: !state.isLive }));
  },
}));
