import { Fragment, useEffect, useMemo, useState } from 'react';
import { useLogsStore } from '../stores/logsStore';
import type { RequestLog, RequestLogFilter } from '../types';
import { formatNumber } from '../utils/format';
import { FileText, ChevronLeft, ChevronRight, Search, X, ChevronDown, ChevronUp, Copy, Code, MessageSquare, RotateCcw } from 'lucide-react';

const getStatusClass = (status: number): string => {
  if (status >= 200 && status < 300) return 'status-2xx';
  if (status >= 400 && status < 500) return 'status-4xx';
  if (status >= 500) return 'status-5xx';
  return '';
};

function CollapsibleBody({
  label,
  icon,
  sectionKey,
  content,
  openSections,
  toggleSection,
}: {
  label: string;
  icon: React.ReactNode;
  sectionKey: string;
  content: string;
  openSections: Record<string, boolean>;
  toggleSection: (key: string) => void;
}) {
  const isOpen = !!openSections[sectionKey];

  const formatted = useMemo(() => {
    try {
      return JSON.stringify(JSON.parse(content), null, 2);
    } catch {
      return content;
    }
  }, [content]);

  return (
    <div className="log-body-section">
      <button
        className="log-body-toggle"
        onClick={(e) => { e.stopPropagation(); toggleSection(sectionKey); }}
      >
        {icon}
        <span>{label}</span>
        {isOpen ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
      </button>
      {isOpen && (
        <pre className="log-body-pre">{formatted}</pre>
      )}
    </div>
  );
}

function LogDetail({
  log,
  openSections,
  toggleSection,
}: {
  log: RequestLog;
  openSections: Record<string, boolean>;
  toggleSection: (key: string) => void;
}) {
  return (
    <div className="log-detail">
      {/* Metadata grid */}
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
        {log.tenant_id && (
          <div className="log-detail-item">
            <span className="log-detail-label">Tenant</span>
            <span className="log-detail-value">{log.tenant_id}</span>
          </div>
        )}
        {log.credential_name && (
          <div className="log-detail-item">
            <span className="log-detail-label">Credential</span>
            <span className="log-detail-value text-mono">{log.credential_name}</span>
          </div>
        )}
        {log.requested_model && log.requested_model !== log.model && (
          <div className="log-detail-item">
            <span className="log-detail-label">Requested Model</span>
            <span className="log-detail-value text-mono">{log.requested_model}</span>
          </div>
        )}
        {log.client_region && (
          <div className="log-detail-item">
            <span className="log-detail-label">Client Region</span>
            <span className="log-detail-value">{log.client_region}</span>
          </div>
        )}
        {log.total_attempts > 1 && (
          <div className="log-detail-item">
            <span className="log-detail-label">Total Attempts</span>
            <span className="log-detail-value">{log.total_attempts}</span>
          </div>
        )}
        {(log.usage?.cache_read_tokens ?? 0) > 0 && (
          <div className="log-detail-item">
            <span className="log-detail-label">Cache Read Tokens</span>
            <span className="log-detail-value">{log.usage?.cache_read_tokens?.toLocaleString()}</span>
          </div>
        )}
        {(log.usage?.cache_creation_tokens ?? 0) > 0 && (
          <div className="log-detail-item">
            <span className="log-detail-label">Cache Write Tokens</span>
            <span className="log-detail-value">{log.usage?.cache_creation_tokens?.toLocaleString()}</span>
          </div>
        )}
      </div>

      {/* Error with error_type */}
      {log.error && (
        <div className="log-detail-error">
          <span className="log-detail-label">
            Error
            {log.error_type && (
              <span className="log-error-type-badge">{log.error_type}</span>
            )}
          </span>
          <pre className="log-error-pre">{log.error}</pre>
        </div>
      )}

      {/* Retry attempts timeline */}
      {log.attempts && log.attempts.length > 1 && (
        <div className="log-attempts-section">
          <div className="log-detail-label" style={{ marginBottom: 8, display: 'flex', alignItems: 'center', gap: 6 }}>
            <RotateCcw size={12} />
            Retry Attempts ({log.attempts.length})
          </div>
          <div className="log-attempts-timeline">
            {log.attempts.map((attempt) => (
              <div key={attempt.attempt_index} className="log-attempt-item">
                <div className="log-attempt-index">#{attempt.attempt_index + 1}</div>
                <div className="log-attempt-details">
                  <span className="type-badge" style={{ marginRight: 4 }}>{attempt.provider}</span>
                  <span className="text-mono" style={{ fontSize: '0.8rem', marginRight: 8 }}>{attempt.model}</span>
                  {attempt.credential_name && (
                    <span className="text-mono" style={{ fontSize: '0.75rem', opacity: 0.6, marginRight: 8 }}>
                      {attempt.credential_name}
                    </span>
                  )}
                  {attempt.status != null && (
                    <span className={`status-code ${getStatusClass(attempt.status)}`} style={{ marginRight: 8 }}>
                      {attempt.status}
                    </span>
                  )}
                  <span style={{ fontSize: '0.8rem', opacity: 0.7 }}>{attempt.latency_ms}ms</span>
                </div>
                {attempt.error && (
                  <div className="log-attempt-error">
                    {attempt.error_type && <span className="log-error-type-badge">{attempt.error_type}</span>}
                    <span>{attempt.error}</span>
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Stream content preview */}
      {log.stream && log.stream_content_preview && (
        <CollapsibleBody
          label="Stream Content Preview"
          icon={<MessageSquare size={14} />}
          sectionKey={`${log.request_id}-stream-preview`}
          content={log.stream_content_preview}
          openSections={openSections}
          toggleSection={toggleSection}
        />
      )}

      {/* Request body */}
      {log.request_body && (
        <CollapsibleBody
          label="Request Body"
          icon={<Code size={14} />}
          sectionKey={`${log.request_id}-req-body`}
          content={log.request_body}
          openSections={openSections}
          toggleSection={toggleSection}
        />
      )}

      {/* Upstream request body (translated) */}
      {log.upstream_request_body && (
        <CollapsibleBody
          label="Upstream Request Body (translated)"
          icon={<Code size={14} />}
          sectionKey={`${log.request_id}-upstream-body`}
          content={log.upstream_request_body}
          openSections={openSections}
          toggleSection={toggleSection}
        />
      )}

      {/* Response body (non-streaming) */}
      {!log.stream && log.response_body && (
        <CollapsibleBody
          label="Response Body"
          icon={<MessageSquare size={14} />}
          sectionKey={`${log.request_id}-resp-body`}
          content={log.response_body}
          openSections={openSections}
          toggleSection={toggleSection}
        />
      )}
    </div>
  );
}

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

  const formatCost = (cost: number | null): string => {
    if (cost == null || cost === 0) return '-';
    if (cost < 0.01) return `$${cost.toFixed(6)}`;
    return `$${cost.toFixed(4)}`;
  };

  const formatTokens = (log: typeof logs[0]): string => {
    if (!log.usage) return '-';
    const { input_tokens, output_tokens } = log.usage;
    return `${formatNumber(input_tokens)} / ${formatNumber(output_tokens)}`;
  };

  const toggleExpand = (id: string) => {
    setExpandedId(expandedId === id ? null : id);
    setOpenSections({}); // Reset collapsible state when switching rows
  };

  const [openSections, setOpenSections] = useState<Record<string, boolean>>({});

  const toggleSection = (key: string) => {
    setOpenSections((prev) => ({ ...prev, [key]: !prev[key] }));
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
                <th>Tokens (in/out)</th>
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
                  <Fragment key={log.request_id}>
                    <tr
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
                          {log.stream && <span className="type-badge" style={{ marginRight: 4, opacity: 0.7 }}>stream</span>}
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
                        {formatTokens(log)}
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
                          <LogDetail
                            log={log}
                            openSections={openSections}
                            toggleSection={toggleSection}
                          />
                        </td>
                      </tr>
                    )}
                  </Fragment>
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
