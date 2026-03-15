import { create } from 'zustand';
import { authApi, setSessionSetter } from '../services/api';
import { destroyWebSocketManager } from '../services/websocket';

interface AuthState {
  username: string | null;
  isAuthenticated: boolean;
  isLoading: boolean;
  initialized: boolean;
  error: string | null;
  login: (username: string, password: string) => Promise<void>;
  logout: () => Promise<void>;
  refreshToken: () => Promise<boolean>;
  initialize: () => Promise<void>;
}

function applySession(authenticated: boolean, username?: string | null) {
  if (!authenticated) {
    destroyWebSocketManager();
  }
  useAuthStore.setState((state) => ({
    username: authenticated ? (username ?? state.username) : null,
    isAuthenticated: authenticated,
    initialized: true,
    isLoading: false,
  }));
}

export const useAuthStore = create<AuthState>((set) => ({
  username: null,
  isAuthenticated: false,
  isLoading: true,
  initialized: false,
  error: null,

  initialize: async () => {
    set({ isLoading: true });
    try {
      const response = await authApi.session();
      applySession(response.data.authenticated, response.data.username);
    } catch {
      applySession(false, null);
    }
  },

  login: async (username: string, password: string) => {
    set({ isLoading: true, error: null });
    try {
      const response = await authApi.login(username, password);
      applySession(response.data.authenticated, response.data.username);
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Login failed';
      set({ error: message, isLoading: false, initialized: true, isAuthenticated: false });
      throw err;
    }
  },

  logout: async () => {
    try {
      await authApi.logout();
    } catch {
      // Best effort cookie clear; local state should still be dropped.
    }
    applySession(false, null);
  },

  refreshToken: async () => {
    try {
      const response = await authApi.refresh();
      applySession(response.data.authenticated, response.data.username);
      return true;
    } catch {
      applySession(false, null);
      return false;
    }
  },
}));

setSessionSetter((authenticated) => {
  applySession(authenticated);
});
