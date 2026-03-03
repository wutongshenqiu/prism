import { describe, it, expect, beforeEach, vi } from 'vitest';
import axios from 'axios';

const mockInstance = {
  interceptors: {
    request: { use: vi.fn() },
    response: { use: vi.fn() },
  },
  get: vi.fn(),
  post: vi.fn(),
  patch: vi.fn(),
  delete: vi.fn(),
};

vi.mock('axios', () => ({
  default: {
    create: vi.fn(() => mockInstance),
    post: vi.fn(),
  },
}));

describe('API service', () => {
  beforeEach(() => {
    vi.resetModules();
    localStorage.clear();
  });

  it('creates axios instance with correct base URL', async () => {
    await import('../../services/api');
    expect(axios.create).toHaveBeenCalledWith(
      expect.objectContaining({
        baseURL: '/api/dashboard',
      })
    );
  });

  it('registers request and response interceptors', async () => {
    await import('../../services/api');
    const instance = vi.mocked(axios.create).mock.results[0]?.value;
    expect(instance.interceptors.request.use).toHaveBeenCalled();
    expect(instance.interceptors.response.use).toHaveBeenCalled();
  });
});

describe('routingApi strategy mapping', () => {
  // The mapping is internal to the module — we test the exported routingApi.update
  // by verifying it sends the right backend format

  it('maps round_robin to round-robin', async () => {
    const mod = await import('../../services/api');
    const instance = vi.mocked(axios.create).mock.results[0]?.value;
    vi.mocked(instance.patch).mockResolvedValueOnce({ data: {} });

    await mod.routingApi.update({ strategy: 'round_robin' });

    expect(instance.patch).toHaveBeenCalledWith(
      '/routing',
      expect.objectContaining({ strategy: 'round-robin' })
    );
  });

  it('maps failover to fill-first', async () => {
    const mod = await import('../../services/api');
    const instance = vi.mocked(axios.create).mock.results[0]?.value;
    vi.mocked(instance.patch).mockResolvedValueOnce({ data: {} });

    await mod.routingApi.update({ strategy: 'failover' });

    expect(instance.patch).toHaveBeenCalledWith(
      '/routing',
      expect.objectContaining({ strategy: 'fill-first' })
    );
  });

  it('maps least_latency to least-latency', async () => {
    const mod = await import('../../services/api');
    const instance = vi.mocked(axios.create).mock.results[0]?.value;
    vi.mocked(instance.patch).mockResolvedValueOnce({ data: {} });

    await mod.routingApi.update({ strategy: 'least_latency' });

    expect(instance.patch).toHaveBeenCalledWith(
      '/routing',
      expect.objectContaining({ strategy: 'least-latency' })
    );
  });

  it('converts timeout_ms to max_retry_interval in seconds', async () => {
    const mod = await import('../../services/api');
    const instance = vi.mocked(axios.create).mock.results[0]?.value;
    vi.mocked(instance.patch).mockResolvedValueOnce({ data: {} });

    await mod.routingApi.update({ timeout_ms: 30000 });

    expect(instance.patch).toHaveBeenCalledWith(
      '/routing',
      expect.objectContaining({ max_retry_interval: 30 })
    );
  });

  it('converts retry_count to request_retry', async () => {
    const mod = await import('../../services/api');
    const instance = vi.mocked(axios.create).mock.results[0]?.value;
    vi.mocked(instance.patch).mockResolvedValueOnce({ data: {} });

    await mod.routingApi.update({ retry_count: 5 });

    expect(instance.patch).toHaveBeenCalledWith(
      '/routing',
      expect.objectContaining({ request_retry: 5 })
    );
  });
});

describe('providersApi type mapping', () => {
  it('converts openai_compat to openai-compat on create', async () => {
    const mod = await import('../../services/api');
    const instance = vi.mocked(axios.create).mock.results[0]?.value;
    vi.mocked(instance.post).mockResolvedValueOnce({ data: {} });

    await mod.providersApi.create({
      name: 'test',
      provider_type: 'openai_compat',
      base_url: 'https://api.example.com',
      api_key: 'key',
      enabled: true,
      models: ['model-1'],
    });

    expect(instance.post).toHaveBeenCalledWith(
      '/providers',
      expect.objectContaining({ provider_type: 'openai-compat' })
    );
  });
});
