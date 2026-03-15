export function extractApiErrorMessage(err: unknown, fallback: string): string {
  if (typeof err === 'object' && err !== null) {
    const maybeError = err as {
      message?: unknown;
      response?: { data?: { message?: unknown; error?: unknown } };
    };
    const apiMessage = maybeError.response?.data?.message;
    if (typeof apiMessage === 'string' && apiMessage.trim()) {
      return apiMessage;
    }
    const apiError = maybeError.response?.data?.error;
    if (typeof apiError === 'string' && apiError.trim()) {
      return apiError;
    }
    if (typeof maybeError.message === 'string' && maybeError.message.trim()) {
      return maybeError.message;
    }
  }
  return fallback;
}
