import { useEffect, useCallback, useState } from 'react';
import { getWebSocketManager, destroyWebSocketManager } from '../services/websocket';
import type { ConnectionState } from '../services/websocket';
import { useAuthStore } from '../stores/authStore';
import { useMetricsStore } from '../stores/metricsStore';
import { useLogsStore } from '../stores/logsStore';
import type { MetricsSnapshot, RequestLog, WsMessage } from '../types';

export function useWebSocket(): { connectionState: ConnectionState } {
  const token = useAuthStore((s) => s.token);
  const setSnapshot = useMetricsStore((s) => s.setSnapshot);
  const addLog = useLogsStore((s) => s.addLog);
  const [connectionState, setConnectionState] = useState<ConnectionState>('disconnected');

  // Stable token provider that always reads the latest token
  const tokenProvider = useCallback(
    () => useAuthStore.getState().token,
    [],
  );

  // Token refresher: attempt to refresh the auth token
  const tokenRefresher = useCallback(async (): Promise<string | null> => {
    try {
      await useAuthStore.getState().refreshToken();
      return useAuthStore.getState().token;
    } catch {
      return null;
    }
  }, []);

  useEffect(() => {
    if (!token) return;

    const manager = getWebSocketManager(tokenProvider, tokenRefresher);
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
  }, [token, tokenProvider, tokenRefresher, setSnapshot, addLog]);

  useEffect(() => {
    return () => {
      destroyWebSocketManager();
    };
  }, []);

  return { connectionState };
}
