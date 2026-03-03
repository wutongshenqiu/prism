import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter, Routes, Route } from 'react-router-dom';

vi.mock('../../services/api', () => ({
  authApi: { login: vi.fn(), refresh: vi.fn() },
}));
vi.mock('../../services/websocket', () => ({
  destroyWebSocketManager: vi.fn(),
}));

const { useAuthStore } = await import('../../stores/authStore');
const { default: ProtectedRoute } = await import('../../components/ProtectedRoute');

function renderWithRouter(isAuthenticated: boolean) {
  useAuthStore.setState({
    token: isAuthenticated ? 'test-token' : null,
    isAuthenticated,
    isLoading: false,
    error: null,
  });

  return render(
    <MemoryRouter initialEntries={['/dashboard']}>
      <Routes>
        <Route
          path="/dashboard"
          element={
            <ProtectedRoute>
              <div>Protected Content</div>
            </ProtectedRoute>
          }
        />
        <Route path="/login" element={<div>Login Page</div>} />
      </Routes>
    </MemoryRouter>
  );
}

describe('ProtectedRoute', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('renders children when authenticated', () => {
    renderWithRouter(true);
    expect(screen.getByText('Protected Content')).toBeInTheDocument();
  });

  it('redirects to /login when not authenticated', () => {
    renderWithRouter(false);
    expect(screen.getByText('Login Page')).toBeInTheDocument();
    expect(screen.queryByText('Protected Content')).not.toBeInTheDocument();
  });
});
