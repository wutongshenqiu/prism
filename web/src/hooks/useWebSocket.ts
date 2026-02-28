import { useEffect, useRef } from 'react';
import { getWebSocketManager, destroyWebSocketManager } from '../services/websocket';
import { useAuthStore } from '../stores/authStore';
import { useMetricsStore } from '../stores/metricsStore';
import { useLogsStore } from '../stores/logsStore';
import type { MetricsSnapshot, RequestLog, WsMessage } from '../types';

export function useWebSocket(): void {
  const token = useAuthStore((s) => s.token);
  const setSnapshot = useMetricsStore((s) => s.setSnapshot);
  const addTimeSeriesPoint = useMetricsStore((s) => s.addTimeSeriesPoint);
  const addLog = useLogsStore((s) => s.addLog);
  const connectedRef = useRef(false);

  useEffect(() => {
    if (!token) return;

    const manager = getWebSocketManager(token);

    if (!connectedRef.current) {
      manager.connect();
      connectedRef.current = true;
    }

    const unsubscribe = manager.subscribe((message: WsMessage) => {
      switch (message.type) {
        case 'metrics': {
          const metrics = message.data as MetricsSnapshot;
          setSnapshot(metrics);
          addTimeSeriesPoint({
            timestamp: new Date().toISOString(),
            requests: metrics.requests_per_minute,
            errors: Math.round(metrics.error_rate * metrics.requests_per_minute),
            tokens: metrics.total_tokens,
            latency_ms: metrics.avg_latency_ms,
          });
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
  }, [token, setSnapshot, addTimeSeriesPoint, addLog]);

  useEffect(() => {
    return () => {
      destroyWebSocketManager();
      connectedRef.current = false;
    };
  }, []);
}
