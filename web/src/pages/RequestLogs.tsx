import { useEffect, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { useLogsStore } from '../stores/logsStore';
import LogDrawer from '../components/LogDrawer';
import FilterSelect from '../components/FilterSelect';
import type { RequestLogFilter } from '../types';
import { formatNumber, getStatusClass, formatCost } from '../utils/format';
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
  const filterOptions = useLogsStore((s) => s.filterOptions);
  const fetchFilterOptions = useLogsStore((s) => s.fetchFilterOptions);
  const openDrawer = useLogsStore((s) => s.openDrawer);
  const isLive = useLogsStore((s) => s.isLive);
  const toggleLive = useLogsStore((s) => s.toggleLive);

  const [searchParams, setSearchParams] = useSearchParams();

  // Filter state — initialize from URL search params
  const [filterProvider, setFilterProvider] = useState(searchParams.get('provider') || '');
  const [filterModel, setFilterModel] = useState(searchParams.get('model') || '');
  const [filterStatus, setFilterStatus] = useState(searchParams.get('status') || '');
  const [filterErrorType, setFilterErrorType] = useState(searchParams.get('error_type') || '');
  const [filterKeyword, setFilterKeyword] = useState(searchParams.get('keyword') || '');
  const [filterTenantId, setFilterTenantId] = useState(searchParams.get('tenant_id') || '');
  const [filterApiKeyId, setFilterApiKeyId] = useState(searchParams.get('api_key_id') || '');
  const [filterStream, setFilterStream] = useState(searchParams.get('stream') || '');
  const [filterLatencyMin, setFilterLatencyMin] = useState(searchParams.get('latency_min') || '');
  const [filterLatencyMax, setFilterLatencyMax] = useState(searchParams.get('latency_max') || '');

  // Apply URL-based filters on mount
  useEffect(() => {
    const initialFilters: RequestLogFilter = {};
    if (searchParams.get('provider')) initialFilters.provider = searchParams.get('provider')!;
    if (searchParams.get('model')) initialFilters.model = searchParams.get('model')!;
    if (searchParams.get('status')) initialFilters.status = searchParams.get('status')!;
    if (searchParams.get('error_type')) initialFilters.error_type = searchParams.get('error_type')!;
    if (searchParams.get('keyword')) initialFilters.keyword = searchParams.get('keyword')!;
    if (searchParams.get('tenant_id')) initialFilters.tenant_id = searchParams.get('tenant_id')!;
    if (searchParams.get('api_key_id')) initialFilters.api_key_id = searchParams.get('api_key_id')!;
    if (searchParams.get('stream')) initialFilters.stream = searchParams.get('stream') === 'true';
    if (searchParams.get('latency_min')) initialFilters.latency_min = Number(searchParams.get('latency_min'));
    if (searchParams.get('latency_max')) initialFilters.latency_max = Number(searchParams.get('latency_max'));
    if (Object.keys(initialFilters).length > 0) setFilters(initialFilters);
    fetchLogs();
    fetchFilterOptions();
  }, [fetchLogs, fetchFilterOptions]); // eslint-disable-line react-hooks/exhaustive-deps

  // Auto-open drawer if ?id= is in URL
  useEffect(() => {
    const id = searchParams.get('id');
    if (id) {
      openDrawer(id);
    }
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const handleOpenDrawer = (id: string) => {
    openDrawer(id);
    setSearchParams({ id });
  };

  const handleApplyFilters = () => {
    const filters: RequestLogFilter = {};
    if (filterProvider) filters.provider = filterProvider;
    if (filterModel) filters.model = filterModel;
    if (filterStatus) filters.status = filterStatus;
    if (filterErrorType) filters.error_type = filterErrorType;
    if (filterKeyword) filters.keyword = filterKeyword;
    if (filterTenantId) filters.tenant_id = filterTenantId;
    if (filterApiKeyId) filters.api_key_id = filterApiKeyId;
    if (filterStream) filters.stream = filterStream === 'true';
    if (filterLatencyMin) filters.latency_min = Number(filterLatencyMin);
    if (filterLatencyMax) filters.latency_max = Number(filterLatencyMax);
    setFilters(filters);

    // Sync to URL
    const params = new URLSearchParams();
    Object.entries(filters).forEach(([k, v]) => {
      if (v !== undefined && v !== '') params.set(k, String(v));
    });
    setSearchParams(params, { replace: true });
  };

  const handleClearFilters = () => {
    setFilterProvider('');
    setFilterModel('');
    setFilterStatus('');
    setFilterErrorType('');
    setFilterKeyword('');
    setFilterTenantId('');
    setFilterApiKeyId('');
    setFilterStream('');
    setFilterLatencyMin('');
    setFilterLatencyMax('');
    setFilters({});
    setSearchParams({}, { replace: true });
  };

  const formatTokens = (log: typeof logs[0]): string => {
    if (!log.usage) return '-';
    const { input_tokens, output_tokens } = log.usage;
    return `${formatNumber(input_tokens)} / ${formatNumber(output_tokens)}`;
  };

  const COL_COUNT = 8;

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h2>Request Logs</h2>
          <p className="page-subtitle">{total} total requests</p>
        </div>
        <div className="page-header-actions">
          <button
            className={`btn btn-sm ${isLive ? 'btn-primary' : 'btn-secondary'}`}
            onClick={toggleLive}
          >
            <span className={`live-dot ${isLive ? 'live-dot--active' : ''}`} />
            {isLive ? 'Live' : 'Paused'}
          </button>
        </div>
      </div>

      {/* Filters */}
      <div className="card" style={{ marginBottom: '1.5rem' }}>
        <div className="card-body">
          <div className="filter-bar">
            <div className="filter-group">
              <FilterSelect
                value={filterProvider}
                onChange={setFilterProvider}
                options={filterOptions?.providers ?? []}
                placeholder="All Providers"
              />
              <FilterSelect
                value={filterModel}
                onChange={setFilterModel}
                options={filterOptions?.models ?? []}
                placeholder="All Models"
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
              <FilterSelect
                value={filterErrorType}
                onChange={setFilterErrorType}
                options={filterOptions?.error_types ?? []}
                placeholder="All Errors"
              />
              <select
                value={filterStream}
                onChange={(e) => setFilterStream(e.target.value)}
                className="filter-input"
              >
                <option value="">All Modes</option>
                <option value="true">Stream</option>
                <option value="false">Non-Stream</option>
              </select>
              <input
                type="text"
                placeholder="Tenant ID"
                value={filterTenantId}
                onChange={(e) => setFilterTenantId(e.target.value)}
                className="filter-input"
                style={{ maxWidth: 140 }}
                onKeyDown={(e) => { if (e.key === 'Enter') handleApplyFilters(); }}
              />
              <input
                type="text"
                placeholder="API Key ID"
                value={filterApiKeyId}
                onChange={(e) => setFilterApiKeyId(e.target.value)}
                className="filter-input"
                style={{ maxWidth: 140 }}
                onKeyDown={(e) => { if (e.key === 'Enter') handleApplyFilters(); }}
              />
              <input
                type="number"
                placeholder="Min ms"
                value={filterLatencyMin}
                onChange={(e) => setFilterLatencyMin(e.target.value)}
                className="filter-input"
                style={{ maxWidth: 80 }}
                onKeyDown={(e) => { if (e.key === 'Enter') handleApplyFilters(); }}
              />
              <input
                type="number"
                placeholder="Max ms"
                value={filterLatencyMax}
                onChange={(e) => setFilterLatencyMax(e.target.value)}
                className="filter-input"
                style={{ maxWidth: 80 }}
                onKeyDown={(e) => { if (e.key === 'Enter') handleApplyFilters(); }}
              />
              <div className="search-input-wrapper">
                <Search size={14} className="search-icon" />
                <input
                  type="text"
                  placeholder="Search keyword..."
                  value={filterKeyword}
                  onChange={(e) => setFilterKeyword(e.target.value)}
                  className="filter-input search-input"
                  onKeyDown={(e) => { if (e.key === 'Enter') handleApplyFilters(); }}
                />
              </div>
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
                <th>Client IP</th>
                <th>Provider / Model</th>
                <th>Status</th>
                <th>Latency</th>
                <th>Tokens (in/out)</th>
                <th>Cost</th>
                <th>API Key</th>
              </tr>
            </thead>
            <tbody>
              {isLoading ? (
                <tr>
                  <td colSpan={COL_COUNT} className="table-empty">Loading...</td>
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
                  <tr
                    key={log.request_id}
                    className="log-row"
                    onClick={() => handleOpenDrawer(log.request_id)}
                    style={{ cursor: 'pointer' }}
                  >
                    <td className="text-nowrap" style={{ fontSize: '0.85rem' }}>
                      {new Date(log.timestamp).toLocaleString()}
                    </td>
                    <td className="text-mono" style={{ fontSize: '0.85rem' }}>
                      {log.client_ip || '-'}
                    </td>
                    <td>
                      <div>
                        {log.provider && <span className="type-badge" style={{ marginRight: 4 }}>{log.provider}</span>}
                        {log.stream && <span className="type-badge" style={{ marginRight: 4, opacity: 0.7 }}>stream</span>}
                        <span className="text-mono" style={{ fontSize: '0.85rem' }}>{log.model || '-'}</span>
                      </div>
                    </td>
                    <td>
                      <span className={`status-code ${getStatusClass(log.status)}`}>{log.status}</span>
                    </td>
                    <td className="text-nowrap">{log.latency_ms}ms</td>
                    <td className="text-nowrap" style={{ fontSize: '0.85rem' }}>{formatTokens(log)}</td>
                    <td className="text-nowrap" style={{ fontSize: '0.85rem' }}>{formatCost(log.cost)}</td>
                    <td className="text-mono text-ellipsis" style={{ fontSize: '0.8rem', maxWidth: 140 }} title={log.api_key_id || undefined}>
                      {log.api_key_id || '-'}
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
            <span className="pagination-info">Page {page} of {totalPages}</span>
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

      <LogDrawer />
    </div>
  );
}
