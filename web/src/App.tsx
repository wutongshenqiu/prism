import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { useEffect } from 'react';
import { useAuthStore } from './stores/authStore';
import Layout from './components/Layout';
import ProtectedRoute from './components/ProtectedRoute';
import Login from './pages/Login';
import Overview from './pages/Overview';
import Metrics from './pages/Metrics';
import RequestLogs from './pages/RequestLogs';
import Providers from './pages/Providers';
import AuthKeys from './pages/AuthKeys';
import Routing from './pages/Routing';
import System from './pages/System';
import Logs from './pages/Logs';
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
          <Route index element={<Overview />} />
          <Route path="metrics" element={<Metrics />} />
          <Route path="request-logs" element={<RequestLogs />} />
          <Route path="providers" element={<Providers />} />
          <Route path="auth-keys" element={<AuthKeys />} />
          <Route path="routing" element={<Routing />} />
          <Route path="system" element={<System />} />
          <Route path="logs" element={<Logs />} />
        </Route>
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </BrowserRouter>
  );
}
