/**
 * Format a duration in seconds to a human-readable uptime string.
 * Examples: "3d 2h 45m", "2h 15m", "45m", "30s"
 */
export function formatUptime(seconds: number): string {
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const mins = Math.floor((seconds % 3600) / 60);
  const secs = Math.floor(seconds % 60);

  const parts: string[] = [];
  if (days > 0) parts.push(`${days}d`);
  if (hours > 0) parts.push(`${hours}h`);
  if (mins > 0) parts.push(`${mins}m`);
  if (parts.length === 0) parts.push(`${secs}s`);

  return parts.join(' ');
}

/**
 * Format a number with K/M suffixes for compact display.
 * Examples: 1500 → "1.5K", 2300000 → "2.3M", 42 → "42"
 */
export function formatNumber(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

export function getStatusClass(status: number): string {
  if (status >= 200 && status < 300) return 'status-2xx';
  if (status >= 400 && status < 500) return 'status-4xx';
  if (status >= 500) return 'status-5xx';
  return '';
}

export function formatCost(cost: number | null): string {
  if (cost == null || cost === 0) return '-';
  if (cost < 0.01) return `$${cost.toFixed(6)}`;
  return `$${cost.toFixed(4)}`;
}
