import { create } from 'zustand';
import { logsApi } from '../services/api';
import type { MetricsSnapshot, LogStats, TimeRange } from '../types';

const TIME_RANGE_MS: Record<TimeRange, number> = {
  '5m': 5 * 60_000,
  '15m': 15 * 60_000,
  '1h': 60 * 60_000,
  '6h': 6 * 60 * 60_000,
  '24h': 24 * 60 * 60_000,
};

interface MetricsState {
  snapshot: MetricsSnapshot | null;
  stats: LogStats | null;
  timeRange: TimeRange;
  isLoading: boolean;
  setSnapshot: (snapshot: MetricsSnapshot) => void;
  setTimeRange: (range: TimeRange) => void;
  fetchStats: (range?: TimeRange) => Promise<void>;
}

export const useMetricsStore = create<MetricsState>((set, get) => ({
  snapshot: null,
  stats: null,
  timeRange: '1h',
  isLoading: false,

  setSnapshot: (snapshot) => {
    const prev = get().snapshot;
    if (prev &&
      prev.total_requests === snapshot.total_requests &&
      prev.total_errors === snapshot.total_errors &&
      prev.total_tokens === snapshot.total_tokens &&
      prev.active_providers === snapshot.active_providers &&
      prev.avg_latency_ms === snapshot.avg_latency_ms &&
      prev.error_rate === snapshot.error_rate
    ) return;
    set({ snapshot });
  },

  setTimeRange: (range) => {
    set({ timeRange: range });
    get().fetchStats(range);
  },

  fetchStats: async (range?: TimeRange) => {
    const tr = range ?? get().timeRange;
    const now = Date.now();
    const from = now - TIME_RANGE_MS[tr];

    set({ isLoading: true });
    try {
      const response = await logsApi.stats({ from, to: now });
      set({ stats: response.data, isLoading: false });
    } catch (err) {
      console.error('Failed to fetch stats:', err);
      set({ isLoading: false });
    }
  },
}));
