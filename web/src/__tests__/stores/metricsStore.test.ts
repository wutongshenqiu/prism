import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useMetricsStore } from '../../stores/metricsStore';
import type { MetricsTimeSeries } from '../../types';

// Mock the API
vi.mock('../../services/api', () => ({
  logsApi: {
    stats: vi.fn(),
  },
}));

describe('metricsStore', () => {
  beforeEach(() => {
    // Reset store state
    useMetricsStore.setState({
      snapshot: null,
      timeSeries: [],
      providerDistribution: [],
      latencyBuckets: [],
    });
  });

  it('should initialize with null snapshot', () => {
    const state = useMetricsStore.getState();
    expect(state.snapshot).toBeNull();
    expect(state.timeSeries).toEqual([]);
    expect(state.providerDistribution).toEqual([]);
    expect(state.latencyBuckets).toEqual([]);
  });

  it('should set snapshot', () => {
    const snapshot = {
      total_requests: 100,
      total_errors: 5,
      total_tokens: 5000,
      active_providers: 3,
      requests_per_minute: 10,
      avg_latency_ms: 250,
      error_rate: 0.05,
      uptime_seconds: 3600,
    };
    useMetricsStore.getState().setSnapshot(snapshot);
    expect(useMetricsStore.getState().snapshot).toEqual(snapshot);
  });

  it('should add time series point', () => {
    const point: MetricsTimeSeries = {
      timestamp: '2025-01-01T00:00:00Z',
      requests: 10,
      errors: 1,
      tokens: 500,
      latency_ms: 200,
    };
    useMetricsStore.getState().addTimeSeriesPoint(point);
    expect(useMetricsStore.getState().timeSeries).toHaveLength(1);
    expect(useMetricsStore.getState().timeSeries[0]).toEqual(point);
  });

  it('should cap time series at 60 points', () => {
    const { addTimeSeriesPoint } = useMetricsStore.getState();

    // Add 65 points
    for (let i = 0; i < 65; i++) {
      addTimeSeriesPoint({
        timestamp: `2025-01-01T00:${String(i).padStart(2, '0')}:00Z`,
        requests: i,
        errors: 0,
        tokens: 0,
        latency_ms: 0,
      });
    }

    const series = useMetricsStore.getState().timeSeries;
    expect(series).toHaveLength(60);
    // First point should be index 5 (oldest 5 were shifted out)
    expect(series[0].requests).toBe(5);
    expect(series[59].requests).toBe(64);
  });

  it('should set provider distribution', () => {
    const data = [
      { provider: 'openai', requests: 50, percentage: 0.5 },
      { provider: 'claude', requests: 30, percentage: 0.3 },
    ];
    useMetricsStore.getState().setProviderDistribution(data);
    expect(useMetricsStore.getState().providerDistribution).toEqual(data);
  });

  it('should set latency buckets', () => {
    const data = [
      { range: '0-100ms', count: 20 },
      { range: '100-500ms', count: 50 },
    ];
    useMetricsStore.getState().setLatencyBuckets(data);
    expect(useMetricsStore.getState().latencyBuckets).toEqual(data);
  });

  it('should fetch stats and populate all fields', async () => {
    const { logsApi } = await import('../../services/api');
    const mockData = {
      snapshot: { total_requests: 42, total_errors: 2, total_tokens: 1000, active_providers: 2, requests_per_minute: 5, avg_latency_ms: 150, error_rate: 0.05, uptime_seconds: 1000 },
      time_series: [{ timestamp: 'ts1', requests: 1, errors: 0, tokens: 10, latency_ms: 50 }],
      provider_distribution: [{ provider: 'openai', requests: 42, percentage: 1.0 }],
      latency_buckets: [{ range: '0-100ms', count: 42 }],
    };
    vi.mocked(logsApi.stats).mockResolvedValueOnce({ data: mockData } as never);

    await useMetricsStore.getState().fetchStats();

    const state = useMetricsStore.getState();
    expect(state.snapshot?.total_requests).toBe(42);
    expect(state.timeSeries).toHaveLength(1);
    expect(state.providerDistribution).toHaveLength(1);
    expect(state.latencyBuckets).toHaveLength(1);
  });

  it('should handle fetchStats failure gracefully', async () => {
    const { logsApi } = await import('../../services/api');
    vi.mocked(logsApi.stats).mockRejectedValueOnce(new Error('network error'));
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    await useMetricsStore.getState().fetchStats();

    expect(consoleSpy).toHaveBeenCalled();
    expect(useMetricsStore.getState().snapshot).toBeNull();
    consoleSpy.mockRestore();
  });
});
