interface StatusBadgeProps {
  status: 'healthy' | 'degraded' | 'unhealthy' | 'up' | 'down' | 'active' | 'inactive';
  label?: string;
}

const statusConfig: Record<string, { className: string; defaultLabel: string }> = {
  healthy: { className: 'status-badge--green', defaultLabel: 'Healthy' },
  up: { className: 'status-badge--green', defaultLabel: 'Up' },
  active: { className: 'status-badge--green', defaultLabel: 'Active' },
  degraded: { className: 'status-badge--yellow', defaultLabel: 'Degraded' },
  unhealthy: { className: 'status-badge--red', defaultLabel: 'Unhealthy' },
  down: { className: 'status-badge--red', defaultLabel: 'Down' },
  inactive: { className: 'status-badge--gray', defaultLabel: 'Inactive' },
};

export default function StatusBadge({ status, label }: StatusBadgeProps) {
  const config = statusConfig[status] ?? {
    className: 'status-badge--gray',
    defaultLabel: status,
  };

  return (
    <span className={`status-badge ${config.className}`}>
      <span className="status-badge-dot" />
      {label ?? config.defaultLabel}
    </span>
  );
}
