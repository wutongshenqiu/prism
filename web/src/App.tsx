import { Suspense, lazy, type ReactNode } from 'react';
import { BrowserRouter, Navigate, Route, Routes } from 'react-router-dom';
import { AppShell } from './components/AppShell';
import { ProtectedRoute } from './components/ProtectedRoute';
import { useI18n } from './i18n';

const LoginPage = lazy(async () => ({ default: (await import('./pages/LoginPage')).LoginPage }));
const AuthProfileCallbackPage = lazy(async () => ({
  default: (await import('./pages/AuthProfileCallbackPage')).AuthProfileCallbackPage,
}));
const CommandCenterPage = lazy(async () => ({
  default: (await import('./pages/CommandCenterPage')).CommandCenterPage,
}));
const TrafficLabPage = lazy(async () => ({
  default: (await import('./pages/TrafficLabPage')).TrafficLabPage,
}));
const ProviderAtlasPage = lazy(async () => ({
  default: (await import('./pages/ProviderAtlasPage')).ProviderAtlasPage,
}));
const RouteStudioPage = lazy(async () => ({
  default: (await import('./pages/RouteStudioPage')).RouteStudioPage,
}));
const ChangeStudioPage = lazy(async () => ({
  default: (await import('./pages/ChangeStudioPage')).ChangeStudioPage,
}));

function RouteFallback() {
  const { t } = useI18n();

  return (
    <div className="page-loader">
      <div className="status-message">{t('common.loading')}</div>
    </div>
  );
}

function withSuspense(element: ReactNode) {
  return <Suspense fallback={<RouteFallback />}>{element}</Suspense>;
}

export default function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/login" element={withSuspense(<LoginPage />)} />
        <Route
          element={(
            <ProtectedRoute>
              <AppShell />
            </ProtectedRoute>
          )}
        >
          <Route index element={<Navigate to="/command-center" replace />} />
          <Route path="/provider-atlas/callback" element={withSuspense(<AuthProfileCallbackPage />)} />
          <Route path="/command-center" element={withSuspense(<CommandCenterPage />)} />
          <Route path="/traffic-lab" element={withSuspense(<TrafficLabPage />)} />
          <Route path="/provider-atlas" element={withSuspense(<ProviderAtlasPage />)} />
          <Route path="/route-studio" element={withSuspense(<RouteStudioPage />)} />
          <Route path="/change-studio" element={withSuspense(<ChangeStudioPage />)} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}
