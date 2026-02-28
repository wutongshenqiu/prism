import { create } from 'zustand';
import { logsApi } from '../services/api';
import type { RequestLog, RequestLogFilter } from '../types';

interface LogsState {
  logs: RequestLog[];
  page: number;
  pageSize: number;
  total: number;
  totalPages: number;
  filters: RequestLogFilter;
  isLoading: boolean;
  setFilters: (filters: RequestLogFilter) => void;
  setPage: (page: number) => void;
  fetchLogs: () => Promise<void>;
  addLog: (log: RequestLog) => void;
}

export const useLogsStore = create<LogsState>((set, get) => ({
  logs: [],
  page: 1,
  pageSize: 50,
  total: 0,
  totalPages: 0,
  filters: {},
  isLoading: false,

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

  addLog: (log) =>
    set((state) => {
      const updated = [log, ...state.logs];
      if (updated.length > state.pageSize) {
        updated.pop();
      }
      return { logs: updated, total: state.total + 1 };
    }),
}));
