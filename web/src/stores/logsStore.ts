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

  // Filter options from backend
  filterOptions: FilterOptions | null;

  // Drawer state
  selectedLogId: string | null;
  selectedLog: RequestLog | null;
  isDrawerOpen: boolean;
  isLoadingDetail: boolean;

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

  filterOptions: null,

  selectedLogId: null,
  selectedLog: null,
  isDrawerOpen: false,
  isLoadingDetail: false,

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
    set({ isLoading: true });
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
      console.error('Failed to fetch logs:', err);
      set({ isLoading: false });
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
    set((state) => {
      const updated = [log, ...state.logs];
      if (updated.length > state.pageSize) {
        updated.pop();
      }
      return { logs: updated, total: state.total + 1 };
    });
  },

  openDrawer: async (id: string) => {
    set({ selectedLogId: id, isDrawerOpen: true, isLoadingDetail: true, selectedLog: null });
    try {
      const response = await logsApi.getById(id);
      set({ selectedLog: response.data, isLoadingDetail: false });
    } catch (err) {
      console.error('Failed to fetch log detail:', err);
      set({ isLoadingDetail: false });
    }
  },

  closeDrawer: () => {
    set({ isDrawerOpen: false, selectedLogId: null, selectedLog: null });
  },

  toggleLive: () => {
    set((state) => ({ isLive: !state.isLive }));
  },
}));
