import { useState, useCallback, useRef, useEffect } from 'react';
import { Search, CheckCircle, XCircle, AlertTriangle } from 'lucide-react';
import { routingApi } from '../../services/api';
import type { PreviewRequest, RouteExplanation } from '../../types';

export default function RoutePreview() {
  const [model, setModel] = useState('');
  const [endpoint, setEndpoint] = useState('chat-completions');
  const [tenant, setTenant] = useState('');
  const [region, setRegion] = useState('');
  const [stream, setStream] = useState(false);
  const [result, setResult] = useState<RouteExplanation | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const debounceRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  const doPreview = useCallback(async () => {
    if (!model.trim()) {
      setResult(null);
      return;
    }
    setLoading(true);
    setError('');
    try {
      const req: PreviewRequest = {
        model: model.trim(),
        endpoint,
        stream,
      };
      if (tenant.trim()) req.tenant_id = tenant.trim();
      if (region.trim()) req.region = region.trim();

      const res = await routingApi.preview(req);
      setResult(res.data);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Preview failed');
      setResult(null);
    } finally {
      setLoading(false);
    }
  }, [model, endpoint, tenant, region, stream]);

  // Auto-preview with debounce
  useEffect(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    if (!model.trim()) return;
    debounceRef.current = setTimeout(doPreview, 500);
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, [model, endpoint, tenant, region, stream, doPreview]);

  return (
    <div className="card">
      <div className="card-header">
        <h3>Route Preview</h3>
      </div>
      <div className="card-body">
        <div className="preview-layout">
          {/* Input form */}
          <div className="preview-form">
            <div className="form-group">
              <label>Model</label>
              <input
                className="form-input"
                value={model}
                onChange={(e) => setModel(e.target.value)}
                placeholder="gpt-4, claude-sonnet-4-5, etc."
              />
            </div>
            <div className="form-group">
              <label>Endpoint</label>
              <select
                className="form-select"
                value={endpoint}
                onChange={(e) => setEndpoint(e.target.value)}
              >
                <option value="chat-completions">Chat Completions</option>
                <option value="messages">Messages</option>
                <option value="responses">Responses</option>
              </select>
            </div>
            <div className="form-row">
              <div className="form-group">
                <label>Tenant</label>
                <input
                  className="form-input"
                  value={tenant}
                  onChange={(e) => setTenant(e.target.value)}
                  placeholder="(optional)"
                />
              </div>
              <div className="form-group">
                <label>Region</label>
                <input
                  className="form-input"
                  value={region}
                  onChange={(e) => setRegion(e.target.value)}
                  placeholder="(optional)"
                />
              </div>
            </div>
            <div className="form-group">
              <label className="checkbox-label">
                <input
                  type="checkbox"
                  checked={stream}
                  onChange={(e) => setStream(e.target.checked)}
                />
                Stream
              </label>
            </div>
            <button className="btn btn-primary" onClick={doPreview} disabled={loading || !model.trim()}>
              <Search size={14} />
              {loading ? 'Loading...' : 'Preview'}
            </button>
          </div>

          {/* Result panel */}
          <div className="preview-result">
            {error && (
              <div className="alert alert-error">{error}</div>
            )}
            {!result && !error && (
              <p style={{ color: 'var(--color-text-secondary)', fontStyle: 'italic' }}>
                Enter a model name to preview the routing decision.
              </p>
            )}
            {result && (
              <div className="preview-details">
                <div className="preview-section">
                  <h4>Profile: <code>{result.profile}</code></h4>
                  {result.matched_rule && (
                    <p>Matched rule: <code>{result.matched_rule}</code></p>
                  )}
                  {result.model_chain.length > 0 && (
                    <p>Model chain: <code>{result.model_chain.join(' → ')}</code></p>
                  )}
                </div>

                {result.selected && (
                  <div className="preview-section">
                    <h4><CheckCircle size={14} style={{ color: 'var(--color-success)' }} /> Selected</h4>
                    <div className="preview-route-card preview-route-card--selected">
                      <span className="preview-provider">{result.selected.provider}</span>
                      <span className="preview-credential">{result.selected.credential_name}</span>
                      <span className="preview-model">{result.selected.model}</span>
                      <span className="preview-score">weight: {result.selected.score.weight}</span>
                      {result.selected.score.latency_ms != null && (
                        <span className="preview-score">latency: {result.selected.score.latency_ms.toFixed(1)}ms</span>
                      )}
                    </div>
                  </div>
                )}

                {!result.selected && (
                  <div className="preview-section">
                    <h4><AlertTriangle size={14} style={{ color: 'var(--color-warning)' }} /> No route selected</h4>
                    <p style={{ color: 'var(--color-text-secondary)' }}>No matching credentials found for this request.</p>
                  </div>
                )}

                {result.alternates.length > 0 && (
                  <div className="preview-section">
                    <h4>Alternates ({result.alternates.length})</h4>
                    {result.alternates.map((alt, i) => (
                      <div key={i} className="preview-route-card">
                        <span className="preview-provider">{alt.provider}</span>
                        <span className="preview-credential">{alt.credential_name}</span>
                        <span className="preview-model">{alt.model}</span>
                      </div>
                    ))}
                  </div>
                )}

                {result.rejections.length > 0 && (
                  <div className="preview-section">
                    <h4><XCircle size={14} style={{ color: 'var(--color-error)' }} /> Rejected ({result.rejections.length})</h4>
                    {result.rejections.map((rej, i) => (
                      <div key={i} className="preview-rejection">
                        <code>{rej.candidate}</code>
                        <span className="preview-reason">{rej.reason.replace(/_/g, ' ')}</span>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
