import { useEffect, useState, useCallback } from 'react';
import { tenantsApi } from '../services/api';
import type { TenantSummary, TenantMetricsResponse } from '../types';
import MetricCard from '../components/MetricCard';
import AuthKeys from './AuthKeys';
import {
  Users,
  RefreshCw,
  ChevronDown,
  ChevronUp,
  Activity,
  Coins,
  Hash,
  Layers,
  Key,
} from 'lucide-react';

type Tab = 'tenants' | 'keys';

export default function Tenants() {
  const [activeTab, setActiveTab] = useState<Tab>('tenants');
  const [tenants, setTenants] = useState<TenantSummary[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [detailMetrics, setDetailMetrics] = useState<TenantMetricsResponse | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);

  const fetchTenants = useCallback(async () => {
    try {
      const response = await tenantsApi.list();
      setTenants(response.data);
    } catch (err) {
      console.error('Failed to fetch tenants:', err);
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchTenants();
    const interval = setInterval(fetchTenants, 30000);
    return () => clearInterval(interval);
  }, [fetchTenants]);

  const toggleExpand = async (id: string) => {
    if (expandedId === id) {
      setExpandedId(null);
      setDetailMetrics(null);
      return;
    }
    setExpandedId(id);
    setDetailLoading(true);
    try {
      const res = await tenantsApi.metrics(id);
      setDetailMetrics(res.data);
    } catch (err) {
      console.error('Failed to fetch tenant metrics:', err);
    } finally {
      setDetailLoading(false);
    }
  };

  const totalRequests = tenants.reduce((sum, t) => sum + t.requests, 0);
  const totalTokens = tenants.reduce((sum, t) => sum + t.tokens, 0);
  const totalCost = tenants.reduce((sum, t) => sum + t.cost_usd, 0);

  const formatCost = (usd: number) => usd < 0.01 ? '<$0.01' : `$${usd.toFixed(2)}`;
  const formatTokens = (n: number) => n >= 1_000_000 ? `${(n / 1_000_000).toFixed(1)}M` : n >= 1_000 ? `${(n / 1_000).toFixed(1)}K` : String(n);

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h2>Tenants & Keys</h2>
          <p className="page-subtitle">Access control, per-tenant usage, and API key management</p>
        </div>
        <div className="page-header-actions">
          {activeTab === 'tenants' && (
            <button className="btn btn-secondary" onClick={fetchTenants}>
              <RefreshCw size={16} />
              Refresh
            </button>
          )}
        </div>
      </div>

      {/* Tab navigation */}
      <div className="card" style={{ marginBottom: '1.5rem' }}>
        <div className="card-body" style={{ padding: '0.5rem 1rem' }}>
          <div style={{ display: 'flex', gap: '0.5rem' }}>
            <button
              className={`btn ${activeTab === 'tenants' ? 'btn-primary' : 'btn-ghost'} btn-sm`}
              onClick={() => setActiveTab('tenants')}
            >
              <Users size={14} />
              Tenants
            </button>
            <button
              className={`btn ${activeTab === 'keys' ? 'btn-primary' : 'btn-ghost'} btn-sm`}
              onClick={() => setActiveTab('keys')}
            >
              <Key size={14} />
              Auth Keys
            </button>
          </div>
        </div>
      </div>

      {activeTab === 'keys' ? (
        <AuthKeys embedded />
      ) : (
        <>
          {/* Summary Cards */}
          <div className="metric-grid">
            <MetricCard
              title="Tenants"
              value={String(tenants.length)}
              subtitle="active"
              icon={<Users size={20} />}
              color="blue"
            />
            <MetricCard
              title="Total Requests"
              value={formatTokens(totalRequests)}
              subtitle="across all tenants"
              icon={<Activity size={20} />}
              color="green"
            />
            <MetricCard
              title="Total Tokens"
              value={formatTokens(totalTokens)}
              subtitle="consumed"
              icon={<Hash size={20} />}
              color="purple"
            />
            <MetricCard
              title="Total Cost"
              value={formatCost(totalCost)}
              subtitle="accrued"
              icon={<Coins size={20} />}
              color="orange"
            />
          </div>

          {/* Tenant Table */}
          <div className="card" style={{ marginTop: '1.5rem' }}>
            <div className="card-header">
              <h3>Tenant List</h3>
            </div>
            <div className="table-wrapper">
              <table className="table">
                <thead>
                  <tr>
                    <th>Tenant ID</th>
                    <th style={{ textAlign: 'right' }}>Requests</th>
                    <th style={{ textAlign: 'right' }}>Tokens</th>
                    <th style={{ textAlign: 'right' }}>Cost</th>
                    <th>Details</th>
                  </tr>
                </thead>
                <tbody>
                  {isLoading ? (
                    <tr>
                      <td colSpan={5} className="table-empty">Loading...</td>
                    </tr>
                  ) : tenants.length === 0 ? (
                    <tr>
                      <td colSpan={5} className="table-empty">
                        <div className="empty-state">
                          <Layers size={48} />
                          <p>No tenants found</p>
                          <span className="text-muted">Tenants appear when auth keys with tenant_id are used.</span>
                        </div>
                      </td>
                    </tr>
                  ) : (
                    tenants.map((tenant) => (
                      <>
                        <tr key={tenant.id}>
                          <td className="text-bold text-mono">{tenant.id}</td>
                          <td style={{ textAlign: 'right' }}>{tenant.requests.toLocaleString()}</td>
                          <td style={{ textAlign: 'right' }}>{formatTokens(tenant.tokens)}</td>
                          <td style={{ textAlign: 'right' }}>{formatCost(tenant.cost_usd)}</td>
                          <td>
                            <button
                              className="btn btn-ghost btn-sm"
                              onClick={() => toggleExpand(tenant.id)}
                            >
                              {expandedId === tenant.id ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
                            </button>
                          </td>
                        </tr>
                        {expandedId === tenant.id && (
                          <tr key={`${tenant.id}-detail`}>
                            <td colSpan={5} style={{ background: 'var(--color-bg-secondary)', padding: '1rem' }}>
                              {detailLoading ? (
                                <span className="text-muted">Loading metrics...</span>
                              ) : detailMetrics?.metrics ? (
                                <div style={{ display: 'flex', gap: '2rem' }}>
                                  <div>
                                    <strong>Requests:</strong> {detailMetrics.metrics.requests.toLocaleString()}
                                  </div>
                                  <div>
                                    <strong>Tokens:</strong> {formatTokens(detailMetrics.metrics.tokens)}
                                  </div>
                                  <div>
                                    <strong>Cost:</strong> {formatCost(detailMetrics.metrics.cost_usd)}
                                  </div>
                                </div>
                              ) : (
                                <span className="text-muted">No metrics available for this tenant.</span>
                              )}
                            </td>
                          </tr>
                        )}
                      </>
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
