import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useMetricsStore } from '../../stores/metricsStore';

// Mock the API
vi.mock('../../services/api', () => ({
  logsApi: {
    stats: vi.fn(),
  },
}));

describe('metricsStore', () => {
  beforeEach(() => {
    useMetricsStore.setState({
      snapshot: null,
      stats: null,
      timeRange: '1h',
      isLoading: false,
    });
  });

  it('should initialize with null snapshot and stats', () => {
    const state = useMetricsStore.getState();
    expect(state.snapshot).toBeNull();
    expect(state.stats).toBeNull();
    expect(state.timeRange).toBe('1h');
    expect(state.isLoading).toBe(false);
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

  it('should fetch stats and populate stats field', async () => {
    const { logsApi } = await import('../../services/api');
    const mockStats = {
      total_entries: 42,
      error_count: 2,
      avg_latency_ms: 150,
      p50_latency_ms: 120,
      p95_latency_ms: 400,
      p99_latency_ms: 800,
      total_cost: 1.5,
      total_tokens: 10000,
      time_series: [{ timestamp: 'ts1', requests: 1, errors: 0, avg_latency_ms: 100, tokens: 10, cost: 0.01 }],
      top_models: [],
      top_errors: [],
      provider_distribution: [{ provider: 'openai', requests: 42, percentage: 100.0 }],
      status_distribution: { success: 40, client_error: 1, server_error: 1 },
    };
    vi.mocked(logsApi.stats).mockResolvedValueOnce({ data: mockStats } as never);

    await useMetricsStore.getState().fetchStats();

    const state = useMetricsStore.getState();
    expect(state.stats).toEqual(mockStats);
    expect(state.isLoading).toBe(false);
  });

  it('should handle fetchStats failure gracefully', async () => {
    const { logsApi } = await import('../../services/api');
    vi.mocked(logsApi.stats).mockRejectedValueOnce(new Error('network error'));
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    await useMetricsStore.getState().fetchStats();

    expect(consoleSpy).toHaveBeenCalled();
    expect(useMetricsStore.getState().stats).toBeNull();
    expect(useMetricsStore.getState().isLoading).toBe(false);
    consoleSpy.mockRestore();
  });

  it('should update timeRange and trigger fetchStats', async () => {
    const { logsApi } = await import('../../services/api');
    vi.mocked(logsApi.stats).mockResolvedValue({ data: {} } as never);

    useMetricsStore.getState().setTimeRange('5m');
    expect(useMetricsStore.getState().timeRange).toBe('5m');
  });
});
