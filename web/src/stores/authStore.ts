import { create } from 'zustand';
import { authApi } from '../services/api';
import { destroyWebSocketManager } from '../services/websocket';

interface AuthState {
  token: string | null;
  isAuthenticated: boolean;
  isLoading: boolean;
  error: string | null;
  login: (username: string, password: string) => Promise<void>;
  logout: () => void;
  refreshToken: () => Promise<void>;
  initialize: () => void;
}

// Read token synchronously so ProtectedRoute sees it on first render
const savedToken = localStorage.getItem('auth_token');

export const useAuthStore = create<AuthState>((set) => ({
  token: savedToken,
  isAuthenticated: !!savedToken,
  isLoading: false,
  error: null,

  initialize: () => {
    const token = localStorage.getItem('auth_token');
    if (token) {
      set({ token, isAuthenticated: true });
    }
  },

  login: async (username: string, password: string) => {
    set({ isLoading: true, error: null });
    try {
      const response = await authApi.login(username, password);
      const { token } = response.data;
      localStorage.setItem('auth_token', token);
      set({ token, isAuthenticated: true, isLoading: false });
    } catch (err) {
      const message =
        err instanceof Error ? err.message : 'Login failed';
      set({ error: message, isLoading: false });
      throw err;
    }
  },

  logout: () => {
    localStorage.removeItem('auth_token');
    destroyWebSocketManager();
    set({ token: null, isAuthenticated: false });
  },

  refreshToken: async () => {
    try {
      const response = await authApi.refresh();
      const { token } = response.data;
      localStorage.setItem('auth_token', token);
      set({ token });
    } catch {
      localStorage.removeItem('auth_token');
      set({ token: null, isAuthenticated: false });
    }
  },
}));
