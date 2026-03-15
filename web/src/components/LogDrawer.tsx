import { useEffect } from 'react';
import { useLogsStore } from '../stores/logsStore';
import { X, Copy, RotateCcw, MessageSquare, Code } from 'lucide-react';
import JsonViewer from './JsonViewer';
import { formatNumber, getStatusClass, formatCost } from '../utils/format';

interface LogDrawerProps {
  onClose?: () => void;
}

export default function LogDrawer({ onClose }: LogDrawerProps) {
  const isOpen = useLogsStore((s) => s.isDrawerOpen);
  const log = useLogsStore((s) => s.selectedLog);
  const isLoading = useLogsStore((s) => s.isLoadingDetail);
  const detailError = useLogsStore((s) => s.detailError);
  const closeDrawer = useLogsStore((s) => s.closeDrawer);

  const handleClose = () => {
    closeDrawer();
    onClose?.();
  };

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && isOpen) handleClose();
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [isOpen]); // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <>
      {isOpen && <div className="drawer-overlay" onClick={handleClose} />}
      <div className={`drawer ${isOpen ? 'drawer--open' : ''}`}>
        <div className="drawer-header">
          <h3>Request Detail</h3>
          <button className="btn btn-ghost" onClick={handleClose}>
            <X size={18} />
          </button>
        </div>
        <div className="drawer-body">
          {isLoading && <div className="drawer-loading">Loading...</div>}
          {!isLoading && detailError && <div className="drawer-loading" style={{ color: 'var(--color-danger)' }}>{detailError}</div>}
          {!isLoading && !detailError && !log && <div className="drawer-loading">Not found</div>}
          {!isLoading && log && (
            <>
              {/* Overview */}
              <section className="drawer-section">
                <h4 className="drawer-section-title">Overview</h4>
                <div className="log-detail-grid">
                  <div className="log-detail-item">
                    <span className="log-detail-label">Request ID</span>
                    <span className="log-detail-value text-mono">
                      {log.request_id}
                      <button
                        className="btn btn-ghost btn-sm"
                        onClick={() => navigator.clipboard.writeText(log.request_id)}
                        style={{ padding: '0 4px', marginLeft: 4 }}
                      >
                        <Copy size={12} />
                      </button>
                    </span>
                  </div>
                  <div className="log-detail-item">
                    <span className="log-detail-label">Time</span>
                    <span className="log-detail-value">{new Date(log.timestamp).toLocaleString()}</span>
                  </div>
                  <div className="log-detail-item">
                    <span className="log-detail-label">Method & Path</span>
                    <span className="log-detail-value text-mono">{log.method} {log.path}</span>
                  </div>
                  <div className="log-detail-item">
                    <span className="log-detail-label">Status</span>
                    <span className={`status-code ${getStatusClass(log.status)}`}>{log.status}</span>
                  </div>
                  <div className="log-detail-item">
                    <span className="log-detail-label">Latency</span>
                    <span className="log-detail-value">{log.latency_ms}ms</span>
                  </div>
                  {log.provider && (
                    <div className="log-detail-item">
                      <span className="log-detail-label">Provider</span>
                      <span className="log-detail-value">{log.provider}</span>
                    </div>
                  )}
                  {log.model && (
                    <div className="log-detail-item">
                      <span className="log-detail-label">Model</span>
                      <span className="log-detail-value text-mono">{log.model}</span>
                    </div>
                  )}
                  {log.requested_model && log.requested_model !== log.model && (
                    <div className="log-detail-item">
                      <span className="log-detail-label">Requested Model</span>
                      <span className="log-detail-value text-mono">{log.requested_model}</span>
                    </div>
                  )}
                  {log.credential_name && (
                    <div className="log-detail-item">
                      <span className="log-detail-label">Credential</span>
                      <span className="log-detail-value text-mono">{log.credential_name}</span>
                    </div>
                  )}
                  {log.tenant_id && (
                    <div className="log-detail-item">
                      <span className="log-detail-label">Tenant</span>
                      <span className="log-detail-value">{log.tenant_id}</span>
                    </div>
                  )}
                  {log.client_ip && (
                    <div className="log-detail-item">
                      <span className="log-detail-label">Client IP</span>
                      <span className="log-detail-value text-mono">{log.client_ip}</span>
                    </div>
                  )}
                  {log.client_region && (
                    <div className="log-detail-item">
                      <span className="log-detail-label">Client Region</span>
                      <span className="log-detail-value">{log.client_region}</span>
                    </div>
                  )}
                  {log.api_key_id && (
                    <div className="log-detail-item">
                      <span className="log-detail-label">API Key</span>
                      <span className="log-detail-value text-mono">{log.api_key_id}</span>
                    </div>
                  )}
                  <div className="log-detail-item">
                    <span className="log-detail-label">Stream</span>
                    <span className="log-detail-value">{log.stream ? 'Yes' : 'No'}</span>
                  </div>
                  <div className="log-detail-item">
                    <span className="log-detail-label">Cost</span>
                    <span className="log-detail-value">{formatCost(log.cost)}</span>
                  </div>
                </div>
              </section>

              {/* Token Usage */}
              {log.usage && (
                <section className="drawer-section">
                  <h4 className="drawer-section-title">Token Usage</h4>
                  <div className="log-detail-grid">
                    <div className="log-detail-item">
                      <span className="log-detail-label">Input</span>
                      <span className="log-detail-value">{formatNumber(log.usage.input_tokens)}</span>
                    </div>
                    <div className="log-detail-item">
                      <span className="log-detail-label">Output</span>
                      <span className="log-detail-value">{formatNumber(log.usage.output_tokens)}</span>
                    </div>
                    {(log.usage.cache_read_tokens ?? 0) > 0 && (
                      <div className="log-detail-item">
                        <span className="log-detail-label">Cache Read</span>
                        <span className="log-detail-value">{formatNumber(log.usage.cache_read_tokens)}</span>
                      </div>
                    )}
                    {(log.usage.cache_creation_tokens ?? 0) > 0 && (
                      <div className="log-detail-item">
                        <span className="log-detail-label">Cache Write</span>
                        <span className="log-detail-value">{formatNumber(log.usage.cache_creation_tokens)}</span>
                      </div>
                    )}
                  </div>
                </section>
              )}

              {/* Error */}
              {log.error && (
                <section className="drawer-section">
                  <h4 className="drawer-section-title">
                    Error
                    {log.error_type && <span className="log-error-type-badge">{log.error_type}</span>}
                  </h4>
                  <pre className="log-error-pre">{log.error}</pre>
                </section>
              )}

              {/* Retry Attempts */}
              {log.attempts && log.attempts.length > 1 && (
                <section className="drawer-section">
                  <h4 className="drawer-section-title">
                    <RotateCcw size={14} />
                    Retry Attempts ({log.attempts.length})
                  </h4>
                  <div className="log-attempts-timeline">
                    {log.attempts.map((attempt) => (
                      <div key={attempt.attempt_index} className="log-attempt-item">
                        <div className="log-attempt-index">#{attempt.attempt_index + 1}</div>
                        <div className="log-attempt-details">
                          <span className="type-badge">{attempt.provider}</span>
                          <span className="text-mono" style={{ fontSize: '0.8rem' }}>{attempt.model}</span>
                          {attempt.status != null && (
                            <span className={`status-code ${getStatusClass(attempt.status)}`}>
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
                </section>
              )}

              {/* Bodies */}
              {log.request_body && (
                <section className="drawer-section">
                  <h4 className="drawer-section-title"><Code size={14} /> Request Body</h4>
                  <JsonViewer data={log.request_body} />
                </section>
              )}
              {log.upstream_request_body && (
                <section className="drawer-section">
                  <h4 className="drawer-section-title"><Code size={14} /> Upstream Request Body</h4>
                  <JsonViewer data={log.upstream_request_body} />
                </section>
              )}
              {log.response_body && (
                <section className="drawer-section">
                  <h4 className="drawer-section-title"><MessageSquare size={14} /> Response Body</h4>
                  <JsonViewer data={log.response_body} />
                </section>
              )}
              {log.stream && !log.response_body && log.stream_content_preview && (
                <section className="drawer-section">
                  <h4 className="drawer-section-title"><MessageSquare size={14} /> Stream Content Preview</h4>
                  <JsonViewer data={log.stream_content_preview} />
                </section>
              )}
            </>
          )}
        </div>
      </div>
    </>
  );
}
