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

describe('routingApi', () => {
  it('sends strategy directly to backend', async () => {
    const mod = await import('../../services/api');
    const instance = vi.mocked(axios.create).mock.results[0]?.value;
    vi.mocked(instance.patch).mockResolvedValueOnce({ data: {} });

    await mod.routingApi.update({ strategy: 'round-robin' });

    expect(instance.patch).toHaveBeenCalledWith(
      '/routing',
      expect.objectContaining({ strategy: 'round-robin' })
    );
  });

  it('sends fill-first strategy', async () => {
    const mod = await import('../../services/api');
    const instance = vi.mocked(axios.create).mock.results[0]?.value;
    vi.mocked(instance.patch).mockResolvedValueOnce({ data: {} });

    await mod.routingApi.update({ strategy: 'fill-first' });

    expect(instance.patch).toHaveBeenCalledWith(
      '/routing',
      expect.objectContaining({ strategy: 'fill-first' })
    );
  });

  it('sends request_retry and max_retry_interval directly', async () => {
    const mod = await import('../../services/api');
    const instance = vi.mocked(axios.create).mock.results[0]?.value;
    vi.mocked(instance.patch).mockResolvedValueOnce({ data: {} });

    await mod.routingApi.update({ request_retry: 5, max_retry_interval: 30 });

    expect(instance.patch).toHaveBeenCalledWith(
      '/routing',
      expect.objectContaining({ request_retry: 5, max_retry_interval: 30 })
    );
  });

  it('sends model_strategies and model_fallbacks', async () => {
    const mod = await import('../../services/api');
    const instance = vi.mocked(axios.create).mock.results[0]?.value;
    vi.mocked(instance.patch).mockResolvedValueOnce({ data: {} });

    await mod.routingApi.update({
      model_strategies: { 'claude-*': 'latency-aware' },
      model_fallbacks: { 'gpt-4o': ['gpt-4o-mini'] },
    });

    expect(instance.patch).toHaveBeenCalledWith(
      '/routing',
      expect.objectContaining({
        model_strategies: { 'claude-*': 'latency-aware' },
        model_fallbacks: { 'gpt-4o': ['gpt-4o-mini'] },
      })
    );
  });
});

describe('providersApi type mapping', () => {
  it('passes openai-compat provider type directly', async () => {
    const mod = await import('../../services/api');
    const instance = vi.mocked(axios.create).mock.results[0]?.value;
    vi.mocked(instance.post).mockResolvedValueOnce({ data: {} });

    await mod.providersApi.create({
      provider_type: 'openai-compat',
      api_key: 'key',
      disabled: false,
      models: ['model-1'],
    });

    expect(instance.post).toHaveBeenCalledWith(
      '/providers',
      expect.objectContaining({ provider_type: 'openai-compat' })
    );
  });
});
