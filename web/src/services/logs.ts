import { apiClient } from './api';
import type { RequestLog } from '../types/backend';

export const logsApi = {
  getRequest: async (requestId: string) => {
    try {
      const response = await apiClient.get<RequestLog>(`/logs/${encodeURIComponent(requestId)}`);
      return response.data;
    } catch {
      return null;
    }
  },
};
