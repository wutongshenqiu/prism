import { describe, it, expect, beforeEach, vi } from 'vitest';

// Must mock BEFORE importing the store, because the store module reads
// localStorage at module-evaluation time (line 17 of authStore.ts).
vi.mock('../../services/api', () => ({
  authApi: {
    login: vi.fn(),
    refresh: vi.fn(),
  },
}));
vi.mock('../../services/websocket', () => ({
  destroyWebSocketManager: vi.fn(),
}));

// Dynamically import so mocks are registered first
const { useAuthStore } = await import('../../stores/authStore');

describe('authStore', () => {
  beforeEach(() => {
    localStorage.clear();
    useAuthStore.setState({
      token: null,
      isAuthenticated: false,
      isLoading: false,
      error: null,
    });
  });

  it('should initialize as unauthenticated', () => {
    const state = useAuthStore.getState();
    expect(state.token).toBeNull();
    expect(state.isAuthenticated).toBe(false);
    expect(state.isLoading).toBe(false);
    expect(state.error).toBeNull();
  });

  it('should initialize from localStorage', () => {
    localStorage.setItem('auth_token', 'saved-token');
    useAuthStore.getState().initialize();
    const state = useAuthStore.getState();
    expect(state.token).toBe('saved-token');
    expect(state.isAuthenticated).toBe(true);
  });

  it('should login successfully', async () => {
    const { authApi } = await import('../../services/api');
    vi.mocked(authApi.login).mockResolvedValueOnce({
      data: { token: 'new-jwt-token', expires_in: 3600 },
    } as never);

    await useAuthStore.getState().login('admin', 'password');

    const state = useAuthStore.getState();
    expect(state.token).toBe('new-jwt-token');
    expect(state.isAuthenticated).toBe(true);
    expect(state.isLoading).toBe(false);
    expect(localStorage.getItem('auth_token')).toBe('new-jwt-token');
  });

  it('should handle login failure', async () => {
    const { authApi } = await import('../../services/api');
    vi.mocked(authApi.login).mockRejectedValueOnce(new Error('Invalid credentials'));

    await expect(useAuthStore.getState().login('admin', 'wrong')).rejects.toThrow();

    const state = useAuthStore.getState();
    expect(state.token).toBeNull();
    expect(state.isAuthenticated).toBe(false);
    expect(state.isLoading).toBe(false);
    expect(state.error).toBe('Invalid credentials');
  });

  it('should logout and clear state', async () => {
    const { destroyWebSocketManager } = await import('../../services/websocket');
    useAuthStore.setState({ token: 'tok', isAuthenticated: true });
    localStorage.setItem('auth_token', 'tok');

    useAuthStore.getState().logout();

    expect(useAuthStore.getState().token).toBeNull();
    expect(useAuthStore.getState().isAuthenticated).toBe(false);
    expect(localStorage.getItem('auth_token')).toBeNull();
    expect(destroyWebSocketManager).toHaveBeenCalled();
  });

  it('should refresh token on success', async () => {
    const { authApi } = await import('../../services/api');
    vi.mocked(authApi.refresh).mockResolvedValueOnce({
      data: { token: 'refreshed-token', expires_in: 3600 },
    } as never);
    useAuthStore.setState({ token: 'old-token', isAuthenticated: true });

    await useAuthStore.getState().refreshToken();

    expect(useAuthStore.getState().token).toBe('refreshed-token');
    expect(localStorage.getItem('auth_token')).toBe('refreshed-token');
  });

  it('should logout on refresh failure', async () => {
    const { authApi } = await import('../../services/api');
    vi.mocked(authApi.refresh).mockRejectedValueOnce(new Error('expired'));
    useAuthStore.setState({ token: 'old', isAuthenticated: true });
    localStorage.setItem('auth_token', 'old');

    await useAuthStore.getState().refreshToken();

    expect(useAuthStore.getState().token).toBeNull();
    expect(useAuthStore.getState().isAuthenticated).toBe(false);
    expect(localStorage.getItem('auth_token')).toBeNull();
  });
});
