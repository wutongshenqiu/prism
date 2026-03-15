import { useEffect, useCallback } from 'react';
import { getWebSocketManager, destroyWebSocketManager } from '../services/websocket';
import type { ConnectionState } from '../services/websocket';
import { useAuthStore } from '../stores/authStore';
import { useMetricsStore } from '../stores/metricsStore';
import { useLogsStore } from '../stores/logsStore';
import { useRealtimeStore } from '../stores/realtimeStore';
import type { MetricsSnapshot, RequestLog, WsMessage } from '../types';

export function useWebSocket(): { connectionState: ConnectionState } {
  const isAuthenticated = useAuthStore((s) => s.isAuthenticated);
  const setSnapshot = useMetricsStore((s) => s.setSnapshot);
  const addLog = useLogsStore((s) => s.addLog);
  const connectionState = useRealtimeStore((s) => s.connectionState);
  const setConnectionState = useRealtimeStore((s) => s.setConnectionState);

  // Stable session provider that always reads the latest auth state
  const sessionProvider = useCallback(
    () => useAuthStore.getState().isAuthenticated,
    [],
  );

  const sessionRefresher = useCallback(async (): Promise<boolean> =>
    useAuthStore.getState().refreshToken()
  , []);

  useEffect(() => {
    if (!isAuthenticated) {
      setConnectionState('disconnected');
      return;
    }

    const manager = getWebSocketManager(sessionProvider, sessionRefresher);
    manager.connect();

    const unsubscribe = manager.subscribe((message: WsMessage) => {
      switch (message.type) {
        case 'metrics': {
          setSnapshot(message.data as MetricsSnapshot);
          break;
        }
        case 'request_log': {
          addLog(message.data as RequestLog);
          break;
        }
      }
    });

    const unsubscribeState = manager.onStateChange(setConnectionState);
    setConnectionState(manager.connectionState);

    return () => {
      unsubscribe();
      unsubscribeState();
    };
  }, [isAuthenticated, sessionProvider, sessionRefresher, setSnapshot, addLog, setConnectionState]);

  useEffect(() => {
    return () => {
      destroyWebSocketManager();
    };
  }, []);

  return { connectionState };
}
