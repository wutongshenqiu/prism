import type { WsMessage } from '../types';

export type ConnectionState = 'connected' | 'connecting' | 'disconnected';
type MessageHandler = (message: WsMessage) => void;
type StateHandler = (state: ConnectionState) => void;
type SessionProvider = () => boolean;
type SessionRefresher = () => Promise<boolean>;

export class WebSocketManager {
  private ws: WebSocket | null = null;
  private handlers: Set<MessageHandler> = new Set();
  private stateHandlers: Set<StateHandler> = new Set();
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 10;
  private baseReconnectDelay = 1000;
  private shouldReconnect = true;
  private sessionProvider: SessionProvider;
  private sessionRefresher: SessionRefresher | null = null;
  private _connectionState: ConnectionState = 'disconnected';

  constructor(sessionProvider: SessionProvider, sessionRefresher?: SessionRefresher) {
    this.sessionProvider = sessionProvider;
    this.sessionRefresher = sessionRefresher ?? null;
  }

  private setConnectionState(state: ConnectionState): void {
    this._connectionState = state;
    this.stateHandlers.forEach((h) => h(state));
  }

  private buildUrl(): string | null {
    if (!this.sessionProvider()) return null;
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const host = window.location.host;
    return `${protocol}//${host}/ws/dashboard`;
  }

  connect(): void {
    if (this.ws?.readyState === WebSocket.OPEN) return;

    const url = this.buildUrl();
    if (!url) {
      console.warn('[WS] No active dashboard session, skipping connect');
      this.setConnectionState('disconnected');
      return;
    }

    this.setConnectionState('connecting');

    try {
      this.ws = new WebSocket(url);

      this.ws.onopen = () => {
        console.log('[WS] Connected');
        this.reconnectAttempts = 0;
        this.setConnectionState('connected');
      };

      this.ws.onmessage = (event) => {
        try {
          const message: WsMessage = JSON.parse(event.data);
          this.handlers.forEach((handler) => handler(message));
        } catch (err) {
          console.error('[WS] Failed to parse message:', err);
        }
      };

      this.ws.onclose = (event) => {
        console.log('[WS] Disconnected:', event.code, event.reason);
        this.setConnectionState('disconnected');

        // 4001 = server-side token expired; 1008 = policy violation (auth fail)
        if ((event.code === 4001 || event.code === 1008) && this.sessionRefresher) {
          console.warn('[WS] Auth failure, attempting token refresh before reconnect');
          this.sessionRefresher().then((authenticated) => {
            if (authenticated && this.shouldReconnect) {
              this.reconnectAttempts = 0; // Reset since we got a fresh token
              this.scheduleReconnect();
            }
          }).catch(() => {
            console.warn('[WS] Token refresh failed, stopping reconnect');
          });
          return;
        }

        if (this.shouldReconnect) {
          this.scheduleReconnect();
        }
      };

      this.ws.onerror = (error) => {
        console.error('[WS] Error:', error);
      };
    } catch (err) {
      console.error('[WS] Connection failed:', err);
      this.setConnectionState('disconnected');
      this.scheduleReconnect();
    }
  }

  private scheduleReconnect(): void {
    if (this.reconnectAttempts >= this.maxReconnectAttempts) {
      console.warn('[WS] Max reconnect attempts reached');
      return;
    }

    if (!this.sessionProvider()) {
      console.warn('[WS] No dashboard session for reconnect, stopping');
      return;
    }

    const delay = this.baseReconnectDelay * Math.pow(2, this.reconnectAttempts);
    this.reconnectAttempts++;

    this.setConnectionState('connecting');
    console.log(`[WS] Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts})`);

    this.reconnectTimer = setTimeout(() => {
      this.connect();
    }, delay);
  }

  disconnect(): void {
    this.shouldReconnect = false;

    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }

    if (this.ws) {
      this.ws.close(1000, 'Client disconnect');
      this.ws = null;
    }

    this.handlers.clear();
    this.stateHandlers.clear();
    this.setConnectionState('disconnected');
  }

  subscribe(handler: MessageHandler): () => void {
    this.handlers.add(handler);
    return () => {
      this.handlers.delete(handler);
    };
  }

  onStateChange(handler: StateHandler): () => void {
    this.stateHandlers.add(handler);
    return () => {
      this.stateHandlers.delete(handler);
    };
  }

  get connectionState(): ConnectionState {
    return this._connectionState;
  }

  get isConnected(): boolean {
    return this.ws?.readyState === WebSocket.OPEN;
  }
}

let instance: WebSocketManager | null = null;

export function getWebSocketManager(
  sessionProvider: SessionProvider,
  sessionRefresher?: SessionRefresher,
): WebSocketManager {
  if (!instance) {
    instance = new WebSocketManager(sessionProvider, sessionRefresher);
  }
  return instance;
}

export function destroyWebSocketManager(): void {
  if (instance) {
    instance.disconnect();
    instance = null;
  }
}
