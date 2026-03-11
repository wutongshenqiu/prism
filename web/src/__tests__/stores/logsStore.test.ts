import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useLogsStore } from '../../stores/logsStore';
import type { RequestLog } from '../../types';

vi.mock('../../services/api', () => ({
  logsApi: {
    list: vi.fn(),
    getById: vi.fn(),
    filters: vi.fn(),
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
      filterOptions: null,
      selectedLogId: null,
      selectedLog: null,
      isDrawerOpen: false,
      isLoadingDetail: false,
      isLive: true,
    });
  });

  it('should initialize with empty state', () => {
    const state = useLogsStore.getState();
    expect(state.logs).toEqual([]);
    expect(state.page).toBe(1);
    expect(state.total).toBe(0);
    expect(state.isLoading).toBe(false);
    expect(state.isLive).toBe(true);
    expect(state.isDrawerOpen).toBe(false);
  });

  it('should add log to the beginning when live', () => {
    const log = makeLog();
    useLogsStore.getState().addLog(log);

    const state = useLogsStore.getState();
    expect(state.logs).toHaveLength(1);
    expect(state.logs[0]).toEqual(log);
    expect(state.total).toBe(1);
  });

  it('should not add log when paused', () => {
    useLogsStore.setState({ isLive: false });
    useLogsStore.getState().addLog(makeLog());
    expect(useLogsStore.getState().logs).toHaveLength(0);
  });

  it('should prepend new logs and respect pageSize cap', () => {
    const existing = Array.from({ length: 50 }, (_, i) =>
      makeLog({ request_id: `req-${i}` })
    );
    useLogsStore.setState({ logs: existing, pageSize: 50 });

    const newLog = makeLog({ request_id: 'new-req' });
    useLogsStore.getState().addLog(newLog);

    const state = useLogsStore.getState();
    expect(state.logs).toHaveLength(50);
    expect(state.logs[0].request_id).toBe('new-req');
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

  it('should toggle live state', () => {
    expect(useLogsStore.getState().isLive).toBe(true);
    useLogsStore.getState().toggleLive();
    expect(useLogsStore.getState().isLive).toBe(false);
    useLogsStore.getState().toggleLive();
    expect(useLogsStore.getState().isLive).toBe(true);
  });

  it('should open and close drawer', async () => {
    const { logsApi } = await import('../../services/api');
    const log = makeLog({ request_id: 'detail-1' });
    vi.mocked(logsApi.getById).mockResolvedValueOnce({ data: log } as never);

    await useLogsStore.getState().openDrawer('detail-1');

    let state = useLogsStore.getState();
    expect(state.isDrawerOpen).toBe(true);
    expect(state.selectedLogId).toBe('detail-1');
    expect(state.selectedLog).toEqual(log);
    expect(state.isLoadingDetail).toBe(false);

    useLogsStore.getState().closeDrawer();
    state = useLogsStore.getState();
    expect(state.isDrawerOpen).toBe(false);
    expect(state.selectedLogId).toBeNull();
    expect(state.selectedLog).toBeNull();
  });
});
