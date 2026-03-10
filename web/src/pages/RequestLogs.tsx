import { useEffect, useState } from 'react';
import { useLogsStore } from '../stores/logsStore';
import type { RequestLogFilter } from '../types';
import { FileText, ChevronLeft, ChevronRight, Search, X, ChevronDown, ChevronUp, Copy } from 'lucide-react';

export default function RequestLogs() {
  const logs = useLogsStore((s) => s.logs);
  const page = useLogsStore((s) => s.page);
  const totalPages = useLogsStore((s) => s.totalPages);
  const total = useLogsStore((s) => s.total);
  const isLoading = useLogsStore((s) => s.isLoading);
  const fetchLogs = useLogsStore((s) => s.fetchLogs);
  const setPage = useLogsStore((s) => s.setPage);
  const setFilters = useLogsStore((s) => s.setFilters);

  const [filterProvider, setFilterProvider] = useState('');
  const [filterModel, setFilterModel] = useState('');
  const [filterStatus, setFilterStatus] = useState('');
  const [filterDateFrom, setFilterDateFrom] = useState('');
  const [filterDateTo, setFilterDateTo] = useState('');
  const [expandedId, setExpandedId] = useState<string | null>(null);

  useEffect(() => {
    fetchLogs();
  }, [fetchLogs]);

  const handleApplyFilters = () => {
    const filters: RequestLogFilter = {};
    if (filterProvider) filters.provider = filterProvider;
    if (filterModel) filters.model = filterModel;
    if (filterStatus) filters.status = filterStatus;
    if (filterDateFrom) filters.date_from = filterDateFrom;
    if (filterDateTo) filters.date_to = filterDateTo;
    setFilters(filters);
  };

  const handleClearFilters = () => {
    setFilterProvider('');
    setFilterModel('');
    setFilterStatus('');
    setFilterDateFrom('');
    setFilterDateTo('');
    setFilters({});
  };

  const getStatusClass = (status: number): string => {
    if (status >= 200 && status < 300) return 'status-2xx';
    if (status >= 400 && status < 500) return 'status-4xx';
    if (status >= 500) return 'status-5xx';
    return '';
  };

  const formatCost = (cost: number | null): string => {
    if (cost == null || cost === 0) return '-';
    if (cost < 0.01) return `$${cost.toFixed(6)}`;
    return `$${cost.toFixed(4)}`;
  };

  const toggleExpand = (id: string) => {
    setExpandedId(expandedId === id ? null : id);
  };

  const COL_COUNT = 9;

  return (
    <div className="page">
      <div className="page-header">
        <h2>Request Logs</h2>
        <p className="page-subtitle">{total} total requests</p>
      </div>

      {/* Filters */}
      <div className="card" style={{ marginBottom: '1.5rem' }}>
        <div className="card-body">
          <div className="filter-bar">
            <div className="filter-group">
              <input
                type="text"
                placeholder="Provider"
                value={filterProvider}
                onChange={(e) => setFilterProvider(e.target.value)}
                className="filter-input"
              />
              <input
                type="text"
                placeholder="Model"
                value={filterModel}
                onChange={(e) => setFilterModel(e.target.value)}
                className="filter-input"
              />
              <select
                value={filterStatus}
                onChange={(e) => setFilterStatus(e.target.value)}
                className="filter-input"
              >
                <option value="">All Status</option>
                <option value="2xx">2xx Success</option>
                <option value="4xx">4xx Client Error</option>
                <option value="5xx">5xx Server Error</option>
              </select>
              <input
                type="date"
                value={filterDateFrom}
                onChange={(e) => setFilterDateFrom(e.target.value)}
                className="filter-input"
                placeholder="From"
              />
              <input
                type="date"
                value={filterDateTo}
                onChange={(e) => setFilterDateTo(e.target.value)}
                className="filter-input"
                placeholder="To"
              />
            </div>
            <div className="filter-actions">
              <button className="btn btn-primary btn-sm" onClick={handleApplyFilters}>
                <Search size={14} />
                Search
              </button>
              <button className="btn btn-secondary btn-sm" onClick={handleClearFilters}>
                <X size={14} />
                Clear
              </button>
            </div>
          </div>
        </div>
      </div>

      {/* Table */}
      <div className="card">
        <div className="table-wrapper">
          <table className="table">
            <thead>
              <tr>
                <th style={{ width: 32 }}></th>
                <th>Time</th>
                <th>Client IP</th>
                <th>Provider / Model</th>
                <th>Status</th>
                <th>Latency</th>
                <th>Tokens</th>
                <th>Cost</th>
                <th>API Key</th>
              </tr>
            </thead>
            <tbody>
              {isLoading ? (
                <tr>
                  <td colSpan={COL_COUNT} className="table-empty">
                    Loading...
                  </td>
                </tr>
              ) : logs.length === 0 ? (
                <tr>
                  <td colSpan={COL_COUNT} className="table-empty">
                    <div className="empty-state">
                      <FileText size={48} />
                      <p>No request logs found</p>
                    </div>
                  </td>
                </tr>
              ) : (
                logs.map((log) => (
                  <>
                    <tr
                      key={log.request_id}
                      className={`log-row ${expandedId === log.request_id ? 'log-row-expanded' : ''}`}
                      onClick={() => toggleExpand(log.request_id)}
                      style={{ cursor: 'pointer' }}
                    >
                      <td style={{ width: 32, textAlign: 'center' }}>
                        {expandedId === log.request_id
                          ? <ChevronUp size={14} />
                          : <ChevronDown size={14} />}
                      </td>
                      <td className="text-nowrap" style={{ fontSize: '0.85rem' }}>
                        {new Date(log.timestamp).toLocaleString()}
                      </td>
                      <td className="text-mono" style={{ fontSize: '0.85rem' }}>
                        {log.client_ip || '-'}
                      </td>
                      <td>
                        <div>
                          {log.provider && <span className="type-badge" style={{ marginRight: 4 }}>{log.provider}</span>}
                          <span className="text-mono" style={{ fontSize: '0.85rem' }}>{log.model || '-'}</span>
                        </div>
                      </td>
                      <td>
                        <span className={`status-code ${getStatusClass(log.status)}`}>
                          {log.status}
                        </span>
                      </td>
                      <td className="text-nowrap">{log.latency_ms}ms</td>
                      <td className="text-nowrap" style={{ fontSize: '0.85rem' }}>
                        {log.input_tokens != null || log.output_tokens != null
                          ? `${log.input_tokens ?? 0} / ${log.output_tokens ?? 0}`
                          : '-'}
                      </td>
                      <td className="text-nowrap" style={{ fontSize: '0.85rem' }}>
                        {formatCost(log.cost)}
                      </td>
                      <td className="text-mono" style={{ fontSize: '0.8rem' }}>
                        {log.api_key_id || '-'}
                      </td>
                    </tr>
                    {expandedId === log.request_id && (
                      <tr key={`${log.request_id}-detail`} className="log-detail-row">
                        <td colSpan={COL_COUNT}>
                          <div className="log-detail">
                            <div className="log-detail-grid">
                              <div className="log-detail-item">
                                <span className="log-detail-label">Request ID</span>
                                <span className="log-detail-value text-mono">
                                  {log.request_id}
                                  <button
                                    className="btn btn-ghost btn-sm"
                                    onClick={(e) => { e.stopPropagation(); navigator.clipboard.writeText(log.request_id); }}
                                    style={{ padding: '0 4px', marginLeft: 4 }}
                                  >
                                    <Copy size={12} />
                                  </button>
                                </span>
                              </div>
                              <div className="log-detail-item">
                                <span className="log-detail-label">Method & Path</span>
                                <span className="log-detail-value text-mono">
                                  {log.method} {log.path}
                                </span>
                              </div>
                              <div className="log-detail-item">
                                <span className="log-detail-label">Client IP</span>
                                <span className="log-detail-value text-mono">{log.client_ip || '-'}</span>
                              </div>
                              <div className="log-detail-item">
                                <span className="log-detail-label">Tenant</span>
                                <span className="log-detail-value">{log.tenant_id || '-'}</span>
                              </div>
                              <div className="log-detail-item">
                                <span className="log-detail-label">API Key</span>
                                <span className="log-detail-value text-mono">{log.api_key_id || '-'}</span>
                              </div>
                              <div className="log-detail-item">
                                <span className="log-detail-label">Provider</span>
                                <span className="log-detail-value">{log.provider || '-'}</span>
                              </div>
                              <div className="log-detail-item">
                                <span className="log-detail-label">Model</span>
                                <span className="log-detail-value text-mono">{log.model || '-'}</span>
                              </div>
                              <div className="log-detail-item">
                                <span className="log-detail-label">Status</span>
                                <span className="log-detail-value">
                                  <span className={`status-code ${getStatusClass(log.status)}`}>{log.status}</span>
                                </span>
                              </div>
                              <div className="log-detail-item">
                                <span className="log-detail-label">Latency</span>
                                <span className="log-detail-value">{log.latency_ms}ms</span>
                              </div>
                              <div className="log-detail-item">
                                <span className="log-detail-label">Input Tokens</span>
                                <span className="log-detail-value">{log.input_tokens ?? '-'}</span>
                              </div>
                              <div className="log-detail-item">
                                <span className="log-detail-label">Output Tokens</span>
                                <span className="log-detail-value">{log.output_tokens ?? '-'}</span>
                              </div>
                              <div className="log-detail-item">
                                <span className="log-detail-label">Cost</span>
                                <span className="log-detail-value">{formatCost(log.cost)}</span>
                              </div>
                            </div>
                            {log.error && (
                              <div className="log-detail-error">
                                <span className="log-detail-label">Error</span>
                                <pre className="log-error-pre">{log.error}</pre>
                              </div>
                            )}
                          </div>
                        </td>
                      </tr>
                    )}
                  </>
                ))
              )}
            </tbody>
          </table>
        </div>

        {/* Pagination */}
        {totalPages > 1 && (
          <div className="pagination">
            <button
              className="btn btn-secondary btn-sm"
              disabled={page <= 1}
              onClick={() => setPage(page - 1)}
            >
              <ChevronLeft size={14} />
              Prev
            </button>
            <span className="pagination-info">
              Page {page} of {totalPages}
            </span>
            <button
              className="btn btn-secondary btn-sm"
              disabled={page >= totalPages}
              onClick={() => setPage(page + 1)}
            >
              Next
              <ChevronRight size={14} />
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
