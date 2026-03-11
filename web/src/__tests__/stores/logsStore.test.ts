import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useLogsStore } from '../../stores/logsStore';
import type { RequestLog } from '../../types';

vi.mock('../../services/api', () => ({
  logsApi: {
    list: vi.fn(),
  },
}));

const makeLog = (overrides: Partial<RequestLog> = {}): RequestLog => ({
  request_id: 'req-1',
  timestamp: new Date().toISOString(),
  method: 'POST',
  path: '/v1/chat/completions',
  stream: false,
  requested_model: 'gpt-4',
  provider: 'openai',
  model: 'gpt-4',
  credential_name: null,
  total_attempts: 1,
  status: 200,
  latency_ms: 150,
  usage: { input_tokens: 10, output_tokens: 20, cache_read_tokens: 0, cache_creation_tokens: 0 },
  cost: 0.01,
  api_key_id: null,
  tenant_id: null,
  client_ip: null,
  ...overrides,
});

describe('logsStore', () => {
  beforeEach(() => {
    useLogsStore.setState({
      logs: [],
      page: 1,
      pageSize: 50,
      total: 0,
      totalPages: 0,
      filters: {},
      isLoading: false,
    });
  });

  it('should initialize with empty state', () => {
    const state = useLogsStore.getState();
    expect(state.logs).toEqual([]);
    expect(state.page).toBe(1);
    expect(state.pageSize).toBe(50);
    expect(state.total).toBe(0);
    expect(state.isLoading).toBe(false);
    expect(state.filters).toEqual({});
  });

  it('should add log to the beginning', () => {
    const log = makeLog();
    useLogsStore.getState().addLog(log);

    const state = useLogsStore.getState();
    expect(state.logs).toHaveLength(1);
    expect(state.logs[0]).toEqual(log);
    expect(state.total).toBe(1);
  });

  it('should prepend new logs and respect pageSize cap', () => {
    // Pre-fill with 50 logs
    const existing = Array.from({ length: 50 }, (_, i) =>
      makeLog({ request_id: `req-${i}` })
    );
    useLogsStore.setState({ logs: existing, pageSize: 50 });

    const newLog = makeLog({ request_id: 'new-req' });
    useLogsStore.getState().addLog(newLog);

    const state = useLogsStore.getState();
    expect(state.logs).toHaveLength(50); // capped at pageSize
    expect(state.logs[0].request_id).toBe('new-req'); // prepended
  });

  it('should fetch logs and update state', async () => {
    const { logsApi } = await import('../../services/api');
    vi.mocked(logsApi.list).mockResolvedValueOnce({
      data: {
        data: [makeLog()],
        total: 1,
        total_pages: 1,
        page: 1,
        page_size: 50,
      },
    } as never);

    await useLogsStore.getState().fetchLogs();

    const state = useLogsStore.getState();
    expect(state.logs).toHaveLength(1);
    expect(state.total).toBe(1);
    expect(state.isLoading).toBe(false);
  });

  it('should handle fetch error gracefully', async () => {
    const { logsApi } = await import('../../services/api');
    vi.mocked(logsApi.list).mockRejectedValueOnce(new Error('fail'));
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    await useLogsStore.getState().fetchLogs();

    expect(useLogsStore.getState().isLoading).toBe(false);
    consoleSpy.mockRestore();
  });

  it('should set filters and reset page to 1', async () => {
    const { logsApi } = await import('../../services/api');
    vi.mocked(logsApi.list).mockResolvedValue({
      data: { data: [], total: 0, total_pages: 0, page: 1, page_size: 50 },
    } as never);

    useLogsStore.setState({ page: 3 });
    useLogsStore.getState().setFilters({ provider: 'claude' });

    const state = useLogsStore.getState();
    expect(state.page).toBe(1);
    expect(state.filters.provider).toBe('claude');
  });

  it('should set page and trigger fetch', async () => {
    const { logsApi } = await import('../../services/api');
    vi.mocked(logsApi.list).mockResolvedValue({
      data: { data: [], total: 100, total_pages: 2, page: 2, page_size: 50 },
    } as never);

    useLogsStore.getState().setPage(2);
    expect(useLogsStore.getState().page).toBe(2);
  });
});
