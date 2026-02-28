import type { ReactNode } from 'react';

interface MetricCardProps {
  title: string;
  value: string | number;
  subtitle?: string;
  icon: ReactNode;
  trend?: 'up' | 'down' | 'neutral';
  trendValue?: string;
  color?: 'blue' | 'green' | 'red' | 'purple' | 'orange';
}

export default function MetricCard({
  title,
  value,
  subtitle,
  icon,
  trend,
  trendValue,
  color = 'blue',
}: MetricCardProps) {
  return (
    <div className="metric-card">
      <div className="metric-card-header">
        <span className="metric-card-title">{title}</span>
        <div className={`metric-card-icon metric-card-icon--${color}`}>
          {icon}
        </div>
      </div>
      <div className="metric-card-value">{value}</div>
      <div className="metric-card-footer">
        {trend && trendValue && (
          <span
            className={`metric-card-trend metric-card-trend--${trend}`}
          >
            {trend === 'up' ? '+' : trend === 'down' ? '-' : ''}
            {trendValue}
          </span>
        )}
        {subtitle && (
          <span className="metric-card-subtitle">{subtitle}</span>
        )}
      </div>
    </div>
  );
}
