import { useEffect } from 'react';
import { getWebSocketManager, destroyWebSocketManager } from '../services/websocket';
import { useAuthStore } from '../stores/authStore';
import { useMetricsStore } from '../stores/metricsStore';
import { useLogsStore } from '../stores/logsStore';
import type { MetricsSnapshot, RequestLog, WsMessage } from '../types';

export function useWebSocket(): void {
  const token = useAuthStore((s) => s.token);
  const setSnapshot = useMetricsStore((s) => s.setSnapshot);
  const addLog = useLogsStore((s) => s.addLog);

  useEffect(() => {
    if (!token) return;

    // getWebSocketManager detects token changes and rebuilds the connection
    const manager = getWebSocketManager(token);
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

    return () => {
      unsubscribe();
    };
  }, [token, setSnapshot, addLog]);

  useEffect(() => {
    return () => {
      destroyWebSocketManager();
    };
  }, []);
}
