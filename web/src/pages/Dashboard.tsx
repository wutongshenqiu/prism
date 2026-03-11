import { useEffect, useMemo } from 'react';
import { useMetricsStore } from '../stores/metricsStore';
import MetricCard from '../components/MetricCard';
import TimeRangePicker from '../components/TimeRangePicker';
import TopList from '../components/TopList';
import { formatNumber } from '../utils/format';
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
  PieChart,
  Pie,
  Cell,
  BarChart,
  Bar,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  Legend,
} from 'recharts';

const PIE_COLORS = ['#3b82f6', '#10b981', '#f59e0b', '#ef4444', '#8b5cf6', '#06b6d4', '#ec4899'];

const TOOLTIP_STYLE = {
  background: 'var(--color-bg)',
  border: '1px solid var(--color-border)',
  borderRadius: '8px',
  fontSize: '13px',
};

export default function Dashboard() {
  const snapshot = useMetricsStore((s) => s.snapshot);
  const stats = useMetricsStore((s) => s.stats);
  const timeRange = useMetricsStore((s) => s.timeRange);
  const setTimeRange = useMetricsStore((s) => s.setTimeRange);
  const fetchStats = useMetricsStore((s) => s.fetchStats);

  useEffect(() => {
    fetchStats();
  }, [fetchStats]);

  const topModels = useMemo(() =>
    (stats?.top_models ?? []).slice(0, 5).map((m) => ({
      label: m.model,
      value: `${formatNumber(m.requests)} req`,
      secondary: `${m.avg_latency_ms}ms avg`,
    })), [stats?.top_models]);

  const recentErrors = useMemo(() =>
    (stats?.top_errors ?? []).slice(0, 5).map((e) => ({
      label: e.error_type,
      value: `${e.count}x`,
      secondary: new Date(e.last_seen).toLocaleTimeString(),
    })), [stats?.top_errors]);

  const latencyBuckets = useMemo(() =>
    stats ? [
      { range: 'p50', value: stats.p50_latency_ms },
      { range: 'p95', value: stats.p95_latency_ms },
      { range: 'p99', value: stats.p99_latency_ms },
    ] : [], [stats?.p50_latency_ms, stats?.p95_latency_ms, stats?.p99_latency_ms]);

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h2>Dashboard</h2>
          <p className="page-subtitle">Gateway overview and analytics</p>
        </div>
        <div className="page-header-actions">
          <TimeRangePicker value={timeRange} onChange={setTimeRange} />
        </div>
      </div>

      {/* Metric cards — mix real-time snapshot + stats */}
      <div className="metric-grid">
        <MetricCard
          title="Total Requests"
          value={snapshot ? formatNumber(snapshot.total_requests) : (stats ? formatNumber(stats.total_entries) : '--')}
          subtitle="all time"
          icon={<Activity size={20} />}
          color="blue"
          trend="up"
          trendValue={snapshot ? `${snapshot.requests_per_minute.toFixed(1)}/min` : undefined}
        />
        <MetricCard
          title="Errors"
          value={snapshot ? formatNumber(snapshot.total_errors) : (stats ? formatNumber(stats.error_count) : '--')}
          subtitle={snapshot ? `${(snapshot.error_rate * 100).toFixed(2)}% error rate` : ''}
          icon={<AlertTriangle size={20} />}
          color="red"
        />
        <MetricCard
          title="Total Tokens"
          value={snapshot ? formatNumber(snapshot.total_tokens) : (stats ? formatNumber(stats.total_tokens) : '--')}
          subtitle="consumed"
          icon={<Coins size={20} />}
          color="purple"
        />
        <MetricCard
          title="Total Cost"
          value={stats ? `$${stats.total_cost.toFixed(2)}` : '--'}
          subtitle={`in ${timeRange}`}
          icon={<Coins size={20} />}
          color="orange"
        />
        <MetricCard
          title="Avg Latency"
          value={snapshot ? `${snapshot.avg_latency_ms.toFixed(0)}ms` : (stats ? `${stats.avg_latency_ms}ms` : '--')}
          subtitle="response time"
          icon={<Zap size={20} />}
          color="blue"
        />
        {snapshot ? (
          <MetricCard
            title="Active Providers"
            value={snapshot.active_providers}
            subtitle="connected"
            icon={<Server size={20} />}
            color="green"
          />
        ) : (
          <MetricCard
            title="Uptime"
            value="--"
            subtitle="since last restart"
            icon={<Clock size={20} />}
            color="green"
          />
        )}
      </div>

      {/* Request Trend Chart */}
      <div className="card" style={{ marginTop: '1.5rem' }}>
        <div className="card-header">
          <h3>Request Trend</h3>
        </div>
        <div className="card-body">
          {stats && stats.time_series.length > 0 ? (
            <ResponsiveContainer width="100%" height={300}>
              <LineChart data={stats.time_series}>
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
                <Tooltip contentStyle={TOOLTIP_STYLE} labelFormatter={(v: string) => new Date(v).toLocaleTimeString()} />
                <Legend />
                <Line type="monotone" dataKey="requests" stroke="#3b82f6" strokeWidth={2} dot={false} name="Requests" />
                <Line type="monotone" dataKey="errors" stroke="#ef4444" strokeWidth={2} dot={false} name="Errors" />
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

      {/* Provider Distribution + Top Models */}
      <div className="grid-2col" style={{ marginTop: '1.5rem' }}>
        <div className="card">
          <div className="card-header"><h3>Provider Distribution</h3></div>
          <div className="card-body">
            {stats && stats.provider_distribution.length > 0 ? (
              <ResponsiveContainer width="100%" height={280}>
                <PieChart>
                  <Pie
                    data={stats.provider_distribution}
                    cx="50%"
                    cy="50%"
                    outerRadius={95}
                    dataKey="requests"
                    nameKey="provider"
                    label={({ provider, percentage }: { provider: string; percentage: number }) =>
                      `${provider} (${percentage.toFixed(1)}%)`
                    }
                    labelLine
                  >
                    {stats.provider_distribution.map((_, index) => (
                      <Cell key={index} fill={PIE_COLORS[index % PIE_COLORS.length]} />
                    ))}
                  </Pie>
                  <Tooltip contentStyle={TOOLTIP_STYLE} />
                </PieChart>
              </ResponsiveContainer>
            ) : (
              <div className="empty-state"><p>No provider data yet.</p></div>
            )}
          </div>
        </div>
        <div className="card">
          <div className="card-header"><h3>Top Models</h3></div>
          <div className="card-body">
            <TopList items={topModels} emptyText="No model data yet." />
          </div>
        </div>
      </div>

      {/* Latency Distribution + Recent Errors */}
      <div className="grid-2col" style={{ marginTop: '1.5rem' }}>
        <div className="card">
          <div className="card-header"><h3>Latency Percentiles</h3></div>
          <div className="card-body">
            {latencyBuckets.length > 0 ? (
              <ResponsiveContainer width="100%" height={280}>
                <BarChart data={latencyBuckets}>
                  <CartesianGrid strokeDasharray="3 3" stroke="var(--color-border)" />
                  <XAxis dataKey="range" stroke="var(--color-text-secondary)" fontSize={12} />
                  <YAxis stroke="var(--color-text-secondary)" fontSize={12} unit="ms" />
                  <Tooltip contentStyle={TOOLTIP_STYLE} formatter={(v: number) => [`${v}ms`, 'Latency']} />
                  <Bar dataKey="value" fill="#3b82f6" radius={[4, 4, 0, 0]} name="Latency" />
                </BarChart>
              </ResponsiveContainer>
            ) : (
              <div className="empty-state"><p>No latency data yet.</p></div>
            )}
          </div>
        </div>
        <div className="card">
          <div className="card-header"><h3>Recent Errors</h3></div>
          <div className="card-body">
            <TopList items={recentErrors} emptyText="No errors." />
          </div>
        </div>
      </div>
    </div>
  );
}
