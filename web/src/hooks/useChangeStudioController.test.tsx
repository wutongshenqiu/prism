import { act, renderHook, waitFor } from '@testing-library/react';
import { useChangeStudioController } from './useChangeStudioController';
import { authKeysApi } from '../services/authKeys';
import { tenantsApi } from '../services/tenants';
import type {
  AuthKeysResponse,
  TenantMetricsResponse,
  TenantsResponse,
} from '../types/backend';
import type { ChangeStudioResponse } from '../types/controlPlane';

vi.mock('../services/authKeys', () => ({
  authKeysApi: {
    list: vi.fn(),
    create: vi.fn(),
    update: vi.fn(),
    reveal: vi.fn(),
    remove: vi.fn(),
  },
}));

vi.mock('../services/tenants', () => ({
  tenantsApi: {
    list: vi.fn(),
    metrics: vi.fn(),
  },
}));

vi.mock('../services/config', () => ({
  configApi: {
    raw: vi.fn(),
    validate: vi.fn(),
    apply: vi.fn(),
    reload: vi.fn(),
  },
}));

const mockedAuthKeysApi = vi.mocked(authKeysApi);
const mockedTenantsApi = vi.mocked(tenantsApi);
const emptyAuthKeysResponse: AuthKeysResponse = { auth_keys: [] };
const emptyTenantsResponse: TenantsResponse = { tenants: [] };
const emptyTenantMetricsResponse: TenantMetricsResponse = {
  tenant_id: 'tenant-a',
  metrics: null,
};

function createChangeStudioData(families: string[]): ChangeStudioResponse {
  return {
    registry: families.map((family) => ({
      family,
      record: `${family}-record`,
      state: 'ready',
      state_tone: 'success',
      dependents: '1',
    })),
    publish_facts: [],
    inspector: {
      eyebrow: 'Runtime',
      title: 'Change Studio',
      summary: 'summary',
      sections: [],
      actions: [],
    },
  };
}

describe('useChangeStudioController', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockedAuthKeysApi.list.mockResolvedValue(emptyAuthKeysResponse);
    mockedTenantsApi.list.mockResolvedValue(emptyTenantsResponse);
    mockedTenantsApi.metrics.mockResolvedValue(emptyTenantMetricsResponse);
  });

  it('reconciles selectedFamily when the current family disappears', async () => {
    const reload = vi.fn().mockResolvedValue(undefined);
    const { result, rerender } = renderHook(
      ({ data }) => useChangeStudioController({ data, reload }),
      {
        initialProps: { data: createChangeStudioData(['providers', 'routes']) },
      },
    );

    await waitFor(() => expect(result.current.selectedFamily).toBe('providers'));

    act(() => {
      result.current.setSelectedFamily('routes');
    });
    expect(result.current.selectedFamily).toBe('routes');

    rerender({ data: createChangeStudioData(['auth-keys']) });

    await waitFor(() => expect(result.current.selectedFamily).toBe('auth-keys'));
  });
});
