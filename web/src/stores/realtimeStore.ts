import { create } from 'zustand';
import type { ConnectionState } from '../services/websocket';

interface RealtimeState {
  connectionState: ConnectionState;
  setConnectionState: (state: ConnectionState) => void;
}

export const useRealtimeStore = create<RealtimeState>((set) => ({
  connectionState: 'disconnected',
  setConnectionState: (connectionState) => set({ connectionState }),
}));
