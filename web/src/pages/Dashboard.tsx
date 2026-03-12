import { useEffect, useMemo } from 'react';
import { useMetricsStore } from '../stores/metricsStore';
import MetricCard from '../components/MetricCard';
import TimeRangePicker from '../components/TimeRangePicker';
import TopList from '../components/TopList';
import { formatNumber, formatRate, formatCost } from '../utils/format';
import type { StatusDistribution } from '../types';
import {
  Activity,
  CheckCircle,
  Coins,
  Server,
  Zap,
  Hash,
} from 'lucide-react';
import {
  LineChart,
  Line,
  PieChart,
  Pie,
  Cell,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  Legend,
} from 'recharts';

const PIE_COLORS = ['#3b82f6', '#10b981', '#f59e0b', '#ef4444', '#8b5cf6', '#06b6d4', '#ec4899'];

const RADIAN = Math.PI / 180;
const renderPieLabel = (props: any, formatter: (props: any) => string) => {
  const { cx, cy, midAngle, outerRadius } = props;
  const radius = outerRadius + 25;
  const x = cx + radius * Math.cos(-midAngle * RADIAN);
  const y = cy + radius * Math.sin(-midAngle * RADIAN);
  return (
    <text x={x} y={y} fill="var(--color-text)" fontSize={12}
      textAnchor={x > cx ? 'start' : 'end'} dominantBaseline="central">
      {formatter(props)}
    </text>
  );
};

const STATUS_COLORS: Record<keyof StatusDistribution, string> = { success: '#10b981', client_error: '#f59e0b', server_error: '#ef4444' };

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
  const isLoading = useMetricsStore((s) => s.isLoading);

  const rangeLabel = `in ${timeRange}`;

  useEffect(() => {
    fetchStats();
  }, [fetchStats]);

  const topModels = useMemo(() =>
    (stats?.top_models ?? []).slice(0, 5).map((m) => ({
      label: m.model,
      value: `${formatNumber(m.requests)} req`,
      secondary: `${Math.round(m.avg_latency_ms)}ms · ${formatCost(m.total_cost)}`,
    })), [stats?.top_models]);

  const recentErrors = useMemo(() =>
    (stats?.top_errors ?? []).slice(0, 5).map((e) => ({
      label: e.error_type,
      value: `${e.count}x`,
      secondary: new Date(e.last_seen).toLocaleTimeString(),
    })), [stats?.top_errors]);

  const { successRate, statusPieData } = useMemo(() => {
    if (!stats) return { successRate: null, statusPieData: [] };
    const { success, client_error, server_error } = stats.status_distribution;
    const total = success + client_error + server_error;
    const rate = total === 0 ? null : success / total;
    const pie: { name: string; value: number; color: string }[] = [];
    if (success > 0) pie.push({ name: '2xx Success', value: success, color: STATUS_COLORS.success });
    if (client_error > 0) pie.push({ name: '4xx Client Error', value: client_error, color: STATUS_COLORS.client_error });
    if (server_error > 0) pie.push({ name: '5xx Server Error', value: server_error, color: STATUS_COLORS.server_error });
    return { successRate: rate, statusPieData: pie };
  }, [stats?.status_distribution]);

  const latencySubtitle = useMemo(() => {
    if (stats && stats.p50_latency_ms > 0) {
      return `p50 ${stats.p50_latency_ms} · p95 ${stats.p95_latency_ms} · p99 ${stats.p99_latency_ms}`;
    }
    return 'response time';
  }, [stats?.p50_latency_ms, stats?.p95_latency_ms, stats?.p99_latency_ms]);

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h2>Dashboard</h2>
          <p className="page-subtitle">Gateway overview and analytics</p>
        </div>
        <div className="page-header-actions">
          {isLoading && <span className="text-muted" style={{ fontSize: '0.8rem' }}>Updating...</span>}
          <TimeRangePicker value={timeRange} onChange={setTimeRange} />
        </div>
      </div>

      {/* Metric cards */}
      <div className="metric-grid">
        <MetricCard
          title="Total Requests"
          value={snapshot ? formatNumber(snapshot.total_requests) : (stats ? formatNumber(stats.total_entries) : '--')}
          subtitle={snapshot ? `${snapshot.requests_per_minute.toFixed(1)} req/min` : rangeLabel}
          icon={<Activity size={20} />}
          color="blue"
        />
        <MetricCard
          title="Success Rate"
          value={successRate !== null ? formatRate(successRate) : (snapshot ? formatRate(1 - snapshot.error_rate) : '--')}
          subtitle={rangeLabel}
          icon={<CheckCircle size={20} />}
          color={successRate !== null && successRate < 0.95 ? 'red' : 'green'}
        />
        <MetricCard
          title="Avg Latency"
          value={snapshot ? `${snapshot.avg_latency_ms.toFixed(0)}ms` : (stats ? `${stats.avg_latency_ms}ms` : '--')}
          subtitle={latencySubtitle}
          icon={<Zap size={20} />}
          color="blue"
        />
        <MetricCard
          title="Total Tokens"
          value={snapshot ? formatNumber(snapshot.total_tokens) : (stats ? formatNumber(stats.total_tokens) : '--')}
          subtitle={rangeLabel}
          icon={<Hash size={20} />}
          color="purple"
        />
        <MetricCard
          title="Total Cost"
          value={stats ? formatCost(stats.total_cost) : '--'}
          subtitle={rangeLabel}
          icon={<Coins size={20} />}
          color="orange"
        />
        <MetricCard
          title="Active Providers"
          value={snapshot ? snapshot.active_providers : '--'}
          subtitle="connected"
          icon={<Server size={20} />}
          color="green"
        />
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

      {/* Status Distribution + Top Models */}
      <div className="grid-2col" style={{ marginTop: '1.5rem' }}>
        <div className="card">
          <div className="card-header"><h3>Status Distribution</h3></div>
          <div className="card-body">
            {statusPieData.length > 0 ? (
              <ResponsiveContainer width="100%" height={300}>
                <PieChart>
                  <Pie
                    data={statusPieData}
                    cx="50%"
                    cy="50%"
                    innerRadius={60}
                    outerRadius={85}
                    dataKey="value"
                    nameKey="name"
                    label={(props: any) => renderPieLabel(props, (p) => `${p.name} (${formatNumber(p.value)})`)}
                    labelLine
                  >
                    {statusPieData.map((entry, index) => (
                      <Cell key={index} fill={entry.color} />
                    ))}
                  </Pie>
                  <Tooltip contentStyle={TOOLTIP_STYLE} />
                </PieChart>
              </ResponsiveContainer>
            ) : (
              <div className="empty-state"><p>No status data yet.</p></div>
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

      {/* Provider Distribution + Recent Errors */}
      <div className="grid-2col" style={{ marginTop: '1.5rem' }}>
        <div className="card">
          <div className="card-header"><h3>Provider Distribution</h3></div>
          <div className="card-body">
            {stats && stats.provider_distribution.length > 0 ? (
              <ResponsiveContainer width="100%" height={300}>
                <PieChart>
                  <Pie
                    data={stats.provider_distribution}
                    cx="50%"
                    cy="50%"
                    outerRadius={85}
                    dataKey="requests"
                    nameKey="provider"
                    label={(props: any) => renderPieLabel(props, (p) => `${p.provider} (${p.percentage.toFixed(1)}%)`)}
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
          <div className="card-header"><h3>Recent Errors</h3></div>
          <div className="card-body">
            <TopList items={recentErrors} emptyText="No errors." />
          </div>
        </div>
      </div>
    </div>
  );
}
