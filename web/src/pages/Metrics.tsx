import { useEffect } from 'react';
import { useMetricsStore } from '../stores/metricsStore';
import {
  LineChart,
  Line,
  BarChart,
  Bar,
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
import { BarChart3 } from 'lucide-react';

const PIE_COLORS = ['#3b82f6', '#10b981', '#f59e0b', '#ef4444', '#8b5cf6', '#06b6d4', '#ec4899'];

export default function Metrics() {
  const timeSeries = useMetricsStore((s) => s.timeSeries);
  const providerDistribution = useMetricsStore((s) => s.providerDistribution);
  const latencyBuckets = useMetricsStore((s) => s.latencyBuckets);
  const fetchStats = useMetricsStore((s) => s.fetchStats);

  useEffect(() => {
    fetchStats();
  }, [fetchStats]);

  return (
    <div className="page">
      <div className="page-header">
        <h2>Metrics</h2>
        <p className="page-subtitle">Charts and visualizations</p>
      </div>

      {/* Request Trends */}
      <div className="card">
        <div className="card-header">
          <h3>Request Trends</h3>
        </div>
        <div className="card-body">
          {timeSeries.length > 0 ? (
            <ResponsiveContainer width="100%" height={350}>
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
                />
                <Legend />
                <Line
                  type="monotone"
                  dataKey="requests"
                  stroke="#3b82f6"
                  strokeWidth={2}
                  dot={false}
                  name="Requests"
                />
                <Line
                  type="monotone"
                  dataKey="errors"
                  stroke="#ef4444"
                  strokeWidth={2}
                  dot={false}
                  name="Errors"
                />
                <Line
                  type="monotone"
                  dataKey="latency_ms"
                  stroke="#f59e0b"
                  strokeWidth={2}
                  dot={false}
                  name="Latency (ms)"
                />
              </LineChart>
            </ResponsiveContainer>
          ) : (
            <div className="empty-state">
              <BarChart3 size={48} />
              <p>No time series data available yet.</p>
            </div>
          )}
        </div>
      </div>

      <div className="grid-2col" style={{ marginTop: '1.5rem' }}>
        {/* Latency Distribution */}
        <div className="card">
          <div className="card-header">
            <h3>Latency Distribution</h3>
          </div>
          <div className="card-body">
            {latencyBuckets.length > 0 ? (
              <ResponsiveContainer width="100%" height={300}>
                <BarChart data={latencyBuckets}>
                  <CartesianGrid strokeDasharray="3 3" stroke="var(--color-border)" />
                  <XAxis
                    dataKey="range"
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
                  />
                  <Bar dataKey="count" fill="#3b82f6" radius={[4, 4, 0, 0]} name="Requests" />
                </BarChart>
              </ResponsiveContainer>
            ) : (
              <div className="empty-state">
                <BarChart3 size={48} />
                <p>No latency data available yet.</p>
              </div>
            )}
          </div>
        </div>

        {/* Provider Distribution */}
        <div className="card">
          <div className="card-header">
            <h3>Provider Distribution</h3>
          </div>
          <div className="card-body">
            {providerDistribution.length > 0 ? (
              <ResponsiveContainer width="100%" height={300}>
                <PieChart>
                  <Pie
                    data={providerDistribution}
                    cx="50%"
                    cy="50%"
                    outerRadius={100}
                    dataKey="requests"
                    nameKey="provider"
                    label={({ provider, percentage }: { provider: string; percentage: number }) =>
                      `${provider} (${percentage.toFixed(1)}%)`
                    }
                    labelLine={true}
                  >
                    {providerDistribution.map((_, index) => (
                      <Cell
                        key={`cell-${index}`}
                        fill={PIE_COLORS[index % PIE_COLORS.length]}
                      />
                    ))}
                  </Pie>
                  <Tooltip
                    contentStyle={{
                      background: 'var(--color-bg)',
                      border: '1px solid var(--color-border)',
                      borderRadius: '8px',
                      fontSize: '13px',
                    }}
                  />
                </PieChart>
              </ResponsiveContainer>
            ) : (
              <div className="empty-state">
                <BarChart3 size={48} />
                <p>No provider distribution data yet.</p>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
