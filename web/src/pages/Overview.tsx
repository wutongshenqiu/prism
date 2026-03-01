import { useEffect } from 'react';
import { useMetricsStore } from '../stores/metricsStore';
import MetricCard from '../components/MetricCard';
import {
  Activity,
  AlertTriangle,
  Coins,
  Server,
  Clock,
  Zap,
} from 'lucide-react';
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from 'recharts';

export default function Overview() {
  const snapshot = useMetricsStore((s) => s.snapshot);
  const timeSeries = useMetricsStore((s) => s.timeSeries);
  const fetchStats = useMetricsStore((s) => s.fetchStats);

  useEffect(() => {
    fetchStats();
  }, [fetchStats]);

  const formatNumber = (n: number): string => {
    if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
    if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
    return String(n);
  };

  const formatUptime = (seconds: number): string => {
    const days = Math.floor(seconds / 86400);
    const hours = Math.floor((seconds % 86400) / 3600);
    const mins = Math.floor((seconds % 3600) / 60);
    if (days > 0) return `${days}d ${hours}h`;
    if (hours > 0) return `${hours}h ${mins}m`;
    return `${mins}m`;
  };

  return (
    <div className="page">
      <div className="page-header">
        <h2>Overview</h2>
        <p className="page-subtitle">Real-time gateway statistics</p>
      </div>

      <div className="metric-grid">
        <MetricCard
          title="Total Requests"
          value={snapshot ? formatNumber(snapshot.total_requests) : '--'}
          subtitle="all time"
          icon={<Activity size={20} />}
          color="blue"
          trend="up"
          trendValue={
            snapshot ? `${snapshot.requests_per_minute.toFixed(1)}/min` : undefined
          }
        />
        <MetricCard
          title="Errors"
          value={snapshot ? formatNumber(snapshot.total_errors) : '--'}
          subtitle={
            snapshot
              ? `${(snapshot.error_rate * 100).toFixed(2)}% error rate`
              : ''
          }
          icon={<AlertTriangle size={20} />}
          color="red"
        />
        <MetricCard
          title="Total Tokens"
          value={snapshot ? formatNumber(snapshot.total_tokens) : '--'}
          subtitle="consumed"
          icon={<Coins size={20} />}
          color="purple"
        />
        <MetricCard
          title="Active Providers"
          value={snapshot ? snapshot.active_providers : '--'}
          subtitle="connected"
          icon={<Server size={20} />}
          color="green"
        />
        <MetricCard
          title="Avg Latency"
          value={
            snapshot ? `${snapshot.avg_latency_ms.toFixed(0)}ms` : '--'
          }
          subtitle="response time"
          icon={<Zap size={20} />}
          color="orange"
        />
        <MetricCard
          title="Uptime"
          value={
            snapshot ? formatUptime(snapshot.uptime_seconds) : '--'
          }
          subtitle="since last restart"
          icon={<Clock size={20} />}
          color="blue"
        />
      </div>

      {/* Request Trend Mini Chart */}
      <div className="card" style={{ marginTop: '1.5rem' }}>
        <div className="card-header">
          <h3>Request Trend</h3>
        </div>
        <div className="card-body">
          {timeSeries.length > 0 ? (
            <ResponsiveContainer width="100%" height={300}>
              <LineChart data={timeSeries}>
                <CartesianGrid strokeDasharray="3 3" stroke="var(--color-border)" />
                <XAxis
                  dataKey="timestamp"
                  tickFormatter={(v: string) => {
                    const d = new Date(v);
                    return `${d.getHours().toString().padStart(2, '0')}:${d.getMinutes().toString().padStart(2, '0')}`;
                  }}
                  stroke="var(--color-text-secondary)"
                  fontSize={12}
                />
                <YAxis stroke="var(--color-text-secondary)" fontSize={12} />
                <Tooltip
                  contentStyle={{
                    background: 'var(--color-bg)',
                    border: '1px solid var(--color-border)',
                    borderRadius: '8px',
                    fontSize: '13px',
                  }}
                  labelFormatter={(v: string) => new Date(v).toLocaleTimeString()}
                  formatter={(value: number) => [Number(value.toFixed(1)), undefined]}
                />
                <Line
                  type="monotone"
                  dataKey="requests"
                  stroke="var(--color-primary)"
                  strokeWidth={2}
                  dot={false}
                  name="Requests/min"
                />
                <Line
                  type="monotone"
                  dataKey="errors"
                  stroke="var(--color-danger)"
                  strokeWidth={2}
                  dot={false}
                  name="Errors/min"
                />
              </LineChart>
            </ResponsiveContainer>
          ) : (
            <div className="empty-state">
              <Activity size={48} />
              <p>No data yet. Metrics will appear as requests flow through the gateway.</p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
