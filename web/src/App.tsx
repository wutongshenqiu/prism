import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { useEffect } from 'react';
import { useAuthStore } from './stores/authStore';
import Layout from './components/Layout';
import ProtectedRoute from './components/ProtectedRoute';
import Login from './pages/Login';
import Dashboard from './pages/Dashboard';
import RequestLogs from './pages/RequestLogs';
import Providers from './pages/Providers';
import AuthProfiles from './pages/AuthProfiles';
import Routing from './pages/Routing';
import System from './pages/System';
import Logs from './pages/Logs';
import Config from './pages/Config';
import Tenants from './pages/Tenants';
import Protocols from './pages/Protocols';
import ModelsCapabilities from './pages/ModelsCapabilities';
import Replay from './pages/Replay';
import AuthProfileCallback from './pages/AuthProfileCallback';
import './App.css';

export default function App() {
  const initialize = useAuthStore((s) => s.initialize);

  useEffect(() => {
    initialize();
  }, [initialize]);

  return (
    <BrowserRouter>
      <Routes>
        <Route path="/login" element={<Login />} />
        <Route
          path="/"
          element={
            <ProtectedRoute>
              <Layout />
            </ProtectedRoute>
          }
        >
          <Route index element={<Dashboard />} />
          <Route path="requests" element={<RequestLogs />} />
          <Route path="protocols" element={<Protocols />} />
          <Route path="providers" element={<Providers />} />
          <Route path="auth-profiles" element={<AuthProfiles />} />
          <Route path="auth-profiles/callback" element={<AuthProfileCallback />} />
          <Route path="models" element={<ModelsCapabilities />} />
          <Route path="routing" element={<Routing />} />
          <Route path="replay" element={<Replay />} />
          <Route path="tenants" element={<Tenants />} />
          <Route path="config" element={<Config />} />
          <Route path="system" element={<System />} />
          <Route path="logs" element={<Logs />} />
          {/* Legacy redirects */}
          <Route path="request-logs" element={<Navigate to="/requests" replace />} />
          <Route path="auth-keys" element={<Navigate to="/tenants" replace />} />
        </Route>
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </BrowserRouter>
  );
}
