import { describe, it, expect, beforeEach, vi } from 'vitest';

vi.mock('../../services/api', () => ({
  authApi: {
    login: vi.fn(),
    refresh: vi.fn(),
    session: vi.fn(),
    logout: vi.fn(),
  },
  setSessionSetter: vi.fn(),
}));
vi.mock('../../services/websocket', () => ({
  destroyWebSocketManager: vi.fn(),
}));

const { useAuthStore } = await import('../../stores/authStore');

describe('authStore', () => {
  beforeEach(() => {
    useAuthStore.setState({
      username: null,
      isAuthenticated: false,
      isLoading: false,
      initialized: false,
      error: null,
    });
  });

  it('should initialize as unauthenticated', () => {
    const state = useAuthStore.getState();
    expect(state.username).toBeNull();
    expect(state.isAuthenticated).toBe(false);
    expect(state.error).toBeNull();
  });

  it('should initialize from session endpoint', async () => {
    const { authApi } = await import('../../services/api');
    vi.mocked(authApi.session).mockResolvedValueOnce({
      data: { authenticated: true, username: 'admin' },
    } as never);

    await useAuthStore.getState().initialize();

    const state = useAuthStore.getState();
    expect(state.username).toBe('admin');
    expect(state.isAuthenticated).toBe(true);
    expect(state.initialized).toBe(true);
  });

  it('should login successfully', async () => {
    const { authApi } = await import('../../services/api');
    vi.mocked(authApi.login).mockResolvedValueOnce({
      data: { authenticated: true, username: 'admin', expires_in: 3600 },
    } as never);

    await useAuthStore.getState().login('admin', 'password');

    const state = useAuthStore.getState();
    expect(state.username).toBe('admin');
    expect(state.isAuthenticated).toBe(true);
    expect(state.isLoading).toBe(false);
  });

  it('should handle login failure', async () => {
    const { authApi } = await import('../../services/api');
    vi.mocked(authApi.login).mockRejectedValueOnce(new Error('Invalid credentials'));

    await expect(useAuthStore.getState().login('admin', 'wrong')).rejects.toThrow();

    const state = useAuthStore.getState();
    expect(state.username).toBeNull();
    expect(state.isAuthenticated).toBe(false);
    expect(state.error).toBe('Invalid credentials');
  });

  it('should logout and clear state', async () => {
    const { destroyWebSocketManager } = await import('../../services/websocket');
    const { authApi } = await import('../../services/api');
    vi.mocked(authApi.logout).mockResolvedValueOnce({
      data: { authenticated: false },
    } as never);
    useAuthStore.setState({
      username: 'admin',
      isAuthenticated: true,
      isLoading: false,
      initialized: true,
      error: null,
    });

    await useAuthStore.getState().logout();

    expect(useAuthStore.getState().username).toBeNull();
    expect(useAuthStore.getState().isAuthenticated).toBe(false);
    expect(destroyWebSocketManager).toHaveBeenCalled();
  });

  it('should refresh token on success', async () => {
    const { authApi } = await import('../../services/api');
    vi.mocked(authApi.refresh).mockResolvedValueOnce({
      data: { authenticated: true, username: 'admin', expires_in: 3600 },
    } as never);
    useAuthStore.setState({
      username: 'admin',
      isAuthenticated: true,
      isLoading: false,
      initialized: true,
      error: null,
    });

    await expect(useAuthStore.getState().refreshToken()).resolves.toBe(true);
    expect(useAuthStore.getState().username).toBe('admin');
    expect(useAuthStore.getState().isAuthenticated).toBe(true);
  });

  it('should logout on refresh failure', async () => {
    const { authApi } = await import('../../services/api');
    vi.mocked(authApi.refresh).mockRejectedValueOnce(new Error('expired'));
    useAuthStore.setState({
      username: 'admin',
      isAuthenticated: true,
      isLoading: false,
      initialized: true,
      error: null,
    });

    await expect(useAuthStore.getState().refreshToken()).resolves.toBe(false);
    expect(useAuthStore.getState().username).toBeNull();
    expect(useAuthStore.getState().isAuthenticated).toBe(false);
  });
});
