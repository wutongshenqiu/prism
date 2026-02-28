import { useEffect, useState } from 'react';
import { useLogsStore } from '../stores/logsStore';
import type { RequestLogFilter } from '../types';
import { FileText, ChevronLeft, ChevronRight, Search, X } from 'lucide-react';

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
                <th>Time</th>
                <th>Method</th>
                <th>Path</th>
                <th>Provider</th>
                <th>Model</th>
                <th>Status</th>
                <th>Latency</th>
                <th>Tokens</th>
              </tr>
            </thead>
            <tbody>
              {isLoading ? (
                <tr>
                  <td colSpan={8} className="table-empty">
                    Loading...
                  </td>
                </tr>
              ) : logs.length === 0 ? (
                <tr>
                  <td colSpan={8} className="table-empty">
                    <div className="empty-state">
                      <FileText size={48} />
                      <p>No request logs found</p>
                    </div>
                  </td>
                </tr>
              ) : (
                logs.map((log) => (
                  <tr key={log.id}>
                    <td className="text-nowrap">
                      {new Date(log.timestamp).toLocaleString()}
                    </td>
                    <td>
                      <span className="method-badge">{log.method}</span>
                    </td>
                    <td className="text-mono text-ellipsis" title={log.path}>
                      {log.path}
                    </td>
                    <td>{log.provider}</td>
                    <td className="text-mono">{log.model}</td>
                    <td>
                      <span className={`status-code ${getStatusClass(log.status)}`}>
                        {log.status}
                      </span>
                    </td>
                    <td className="text-nowrap">{log.latency_ms}ms</td>
                    <td className="text-nowrap">
                      {log.input_tokens + log.output_tokens}
                    </td>
                  </tr>
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
