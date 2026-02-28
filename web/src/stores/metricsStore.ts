import { create } from 'zustand';
import { logsApi } from '../services/api';
import type {
  MetricsSnapshot,
  MetricsTimeSeries,
  ProviderDistribution,
  LatencyBucket,
} from '../types';

interface MetricsState {
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

const MAX_TIMESERIES_POINTS = 60;

export const useMetricsStore = create<MetricsState>((set) => ({
  snapshot: null,
  timeSeries: [],
  providerDistribution: [],
  latencyBuckets: [],

  setSnapshot: (snapshot) => set({ snapshot }),

  addTimeSeriesPoint: (point) =>
    set((state) => {
      const updated = [...state.timeSeries, point];
      if (updated.length > MAX_TIMESERIES_POINTS) {
        updated.shift();
      }
      return { timeSeries: updated };
    }),

  setProviderDistribution: (data) => set({ providerDistribution: data }),

  setLatencyBuckets: (data) => set({ latencyBuckets: data }),

  fetchStats: async () => {
    try {
      const response = await logsApi.stats();
      const data = response.data;

      set({
        snapshot: data.snapshot ?? null,
        timeSeries: data.time_series ?? [],
        providerDistribution: data.provider_distribution ?? [],
        latencyBuckets: data.latency_buckets ?? [],
      });
    } catch (err) {
      console.error('Failed to fetch metrics stats:', err);
    }
  },
}));
