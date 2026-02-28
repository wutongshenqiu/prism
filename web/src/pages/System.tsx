import { useEffect, useState, useCallback } from 'react';
import { systemApi, configApi } from '../services/api';
import type { SystemHealth } from '../types';
import StatusBadge from '../components/StatusBadge';
import MetricCard from '../components/MetricCard';
import {
  Activity,
  RefreshCw,
  Power,
  Clock,
  Cpu,
  HardDrive,
  Server,
} from 'lucide-react';

export default function System() {
  const [health, setHealth] = useState<SystemHealth | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [reloading, setReloading] = useState(false);
  const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);

  const fetchHealth = useCallback(async () => {
    try {
      const response = await systemApi.health();
      setHealth(response.data);
    } catch (err) {
      console.error('Failed to fetch health:', err);
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchHealth();
    const interval = setInterval(fetchHealth, 15000);
    return () => clearInterval(interval);
  }, [fetchHealth]);

  const formatUptime = (seconds: number): string => {
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
  };

  const handleReload = async () => {
    if (!window.confirm('Reload the gateway configuration? Active connections will not be affected.')) {
      return;
    }

    setReloading(true);
    setMessage(null);

    try {
      await configApi.reload();
      setMessage({ type: 'success', text: 'Configuration reloaded successfully.' });
      fetchHealth();
    } catch (err) {
      setMessage({
        type: 'error',
        text: err instanceof Error ? err.message : 'Failed to reload configuration',
      });
    } finally {
      setReloading(false);
    }
  };

  if (isLoading) {
    return (
      <div className="page">
        <div className="page-header">
          <h2>System</h2>
        </div>
        <div className="card">
          <div className="card-body">Loading system health...</div>
        </div>
      </div>
    );
  }

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h2>System</h2>
          <p className="page-subtitle">
            Health status and operations
            {health && (
              <> &mdash; <StatusBadge status={health.status} /></>
            )}
          </p>
        </div>
        <div className="page-header-actions">
          <button
            className="btn btn-secondary"
            onClick={fetchHealth}
          >
            <RefreshCw size={16} />
            Refresh
          </button>
          <button
            className="btn btn-primary"
            onClick={handleReload}
            disabled={reloading}
          >
            <Power size={16} />
            {reloading ? 'Reloading...' : 'Reload Config'}
          </button>
        </div>
      </div>

      {message && (
        <div className={`alert alert-${message.type}`} style={{ marginBottom: '1.5rem' }}>
          {message.text}
        </div>
      )}

      {health && (
        <>
          {/* System Metrics */}
          <div className="metric-grid">
            <MetricCard
              title="Uptime"
              value={formatUptime(health.uptime_seconds)}
              subtitle="since last restart"
              icon={<Clock size={20} />}
              color="blue"
            />
            <MetricCard
              title="Version"
              value={health.version}
              subtitle="running"
              icon={<Activity size={20} />}
              color="green"
            />
            <MetricCard
              title="Memory"
              value={`${health.memory_usage_mb.toFixed(0)} MB`}
              subtitle="usage"
              icon={<HardDrive size={20} />}
              color="purple"
            />
            <MetricCard
              title="CPU"
              value={`${health.cpu_usage_percent.toFixed(1)}%`}
              subtitle="usage"
              icon={<Cpu size={20} />}
              color="orange"
            />
          </div>

          {/* Provider Health */}
          <div className="card" style={{ marginTop: '1.5rem' }}>
            <div className="card-header">
              <h3>Provider Health</h3>
            </div>
            <div className="table-wrapper">
              <table className="table">
                <thead>
                  <tr>
                    <th>Provider</th>
                    <th>Status</th>
                    <th>Latency</th>
                    <th>Last Check</th>
                  </tr>
                </thead>
                <tbody>
                  {health.providers.length === 0 ? (
                    <tr>
                      <td colSpan={4} className="table-empty">
                        <div className="empty-state">
                          <Server size={48} />
                          <p>No providers configured</p>
                        </div>
                      </td>
                    </tr>
                  ) : (
                    health.providers.map((provider) => (
                      <tr key={provider.name}>
                        <td className="text-bold">{provider.name}</td>
                        <td>
                          <StatusBadge status={provider.status} />
                        </td>
                        <td>{provider.latency_ms}ms</td>
                        <td>
                          {new Date(provider.last_check).toLocaleString()}
                        </td>
                      </tr>
                    ))
                  )}
                </tbody>
              </table>
            </div>
          </div>
        </>
      )}
    </div>
  );
}
