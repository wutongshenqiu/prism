import { useState } from 'react';
import { routingApi } from '../services/api';
import type { RouteIntrospectionRequest, RouteExplanation } from '../types';
import { formatRejectReason } from '../types';
import {
  PlayCircle,
  Search,
  AlertCircle,
  CheckCircle,
  XCircle,
  ArrowRight,
} from 'lucide-react';

export default function Replay() {
  const [model, setModel] = useState('');
  const [endpoint, setEndpoint] = useState('chat_completions');
  const [sourceFormat, setSourceFormat] = useState('openai');
  const [tenantId, setTenantId] = useState('');
  const [stream, setStream] = useState(false);
  const [region, setRegion] = useState('');
  const [isExplaining, setIsExplaining] = useState(false);
  const [explanation, setExplanation] = useState<RouteExplanation | null>(null);
  const [error, setError] = useState('');

  const handleExplain = async () => {
    if (!model.trim()) {
      setError('Model name is required');
      return;
    }
    setIsExplaining(true);
    setError('');
    setExplanation(null);

    const req: RouteIntrospectionRequest = {
      model: model.trim(),
      endpoint,
      source_format: sourceFormat,
      stream,
    };
    if (tenantId.trim()) req.tenant_id = tenantId.trim();
    if (region.trim()) req.region = region.trim();

    try {
      const res = await routingApi.explain(req);
      setExplanation(res.data);
    } catch (err) {
      if (err && typeof err === 'object' && 'response' in err) {
        const axiosErr = err as { response?: { data?: { message?: string } } };
        setError(axiosErr.response?.data?.message || 'Explain request failed');
      } else {
        setError(err instanceof Error ? err.message : 'Explain request failed');
      }
    } finally {
      setIsExplaining(false);
    }
  };

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h2>Replay</h2>
          <p className="page-subtitle">
            Explain routing decisions and preview request execution
          </p>
        </div>
      </div>

      {/* Request Builder */}
      <div className="card" style={{ marginBottom: '1.5rem' }}>
        <div className="card-header">
          <h3><PlayCircle size={18} style={{ verticalAlign: 'middle', marginRight: '0.5rem' }} />Route Explain</h3>
        </div>
        <div className="card-body">
          <p className="text-muted" style={{ marginBottom: '1rem' }}>
            Enter request parameters to see how the routing planner would handle this request. The explain result uses the same backend logic as the runtime.
          </p>
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(200px, 1fr))', gap: '1rem' }}>
            <div>
              <label className="form-label">Model *</label>
              <input
                type="text"
                className="form-input"
                placeholder="e.g. gpt-4, claude-sonnet-4-5"
                value={model}
                onChange={(e) => setModel(e.target.value)}
                onKeyDown={(e) => { if (e.key === 'Enter') handleExplain(); }}
              />
            </div>
            <div>
              <label className="form-label">Ingress Protocol</label>
              <select className="form-input" value={sourceFormat} onChange={(e) => setSourceFormat(e.target.value)}>
                <option value="openai">OpenAI</option>
                <option value="claude">Claude</option>
                <option value="gemini">Gemini</option>
              </select>
            </div>
            <div>
              <label className="form-label">Endpoint</label>
              <select className="form-input" value={endpoint} onChange={(e) => setEndpoint(e.target.value)}>
                <option value="chat_completions">Chat Completions</option>
                <option value="messages">Messages</option>
                <option value="responses">Responses</option>
                <option value="generate_content">Generate Content</option>
              </select>
            </div>
            <div>
              <label className="form-label">Tenant ID</label>
              <input
                type="text"
                className="form-input"
                placeholder="Optional"
                value={tenantId}
                onChange={(e) => setTenantId(e.target.value)}
              />
            </div>
            <div>
              <label className="form-label">Region</label>
              <input
                type="text"
                className="form-input"
                placeholder="Optional"
                value={region}
                onChange={(e) => setRegion(e.target.value)}
              />
            </div>
            <div>
              <label className="form-label">Stream</label>
              <div style={{ paddingTop: '0.5rem' }}>
                <label style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', cursor: 'pointer' }}>
                  <input type="checkbox" checked={stream} onChange={(e) => setStream(e.target.checked)} />
                  Enable streaming
                </label>
              </div>
            </div>
          </div>
          <div style={{ marginTop: '1rem' }}>
            <button
              className="btn btn-primary"
              onClick={handleExplain}
              disabled={isExplaining || !model.trim()}
            >
              <Search size={16} />
              {isExplaining ? 'Explaining...' : 'Explain Route'}
            </button>
          </div>
        </div>
      </div>

      {error && (
        <div className="alert alert-error" style={{ marginBottom: '1.5rem' }}>
          <AlertCircle size={16} /> {error}
        </div>
      )}

      {/* Explain Result */}
      {explanation && (
        <>
          {/* Selected Route */}
          <div className="card" style={{ marginBottom: '1.5rem' }}>
            <div className="card-header">
              <h3>
                {explanation.selected ? (
                  <><CheckCircle size={18} color="var(--color-success)" style={{ verticalAlign: 'middle', marginRight: '0.5rem' }} />Route Selected</>
                ) : (
                  <><XCircle size={18} color="var(--color-danger)" style={{ verticalAlign: 'middle', marginRight: '0.5rem' }} />No Route Found</>
                )}
              </h3>
            </div>
            <div className="card-body">
              {explanation.selected ? (
                <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(200px, 1fr))', gap: '1rem' }}>
                  <div>
                    <div className="text-muted" style={{ fontSize: '0.75rem', marginBottom: '0.25rem' }}>Provider</div>
                    <div className="text-bold">{explanation.selected.provider}</div>
                  </div>
                  <div>
                    <div className="text-muted" style={{ fontSize: '0.75rem', marginBottom: '0.25rem' }}>Credential</div>
                    <div className="text-mono" style={{ fontSize: '0.85rem' }}>{explanation.selected.credential_name}</div>
                  </div>
                  <div>
                    <div className="text-muted" style={{ fontSize: '0.75rem', marginBottom: '0.25rem' }}>Model</div>
                    <div className="text-mono" style={{ fontSize: '0.85rem' }}>{explanation.selected.model}</div>
                  </div>
                  <div>
                    <div className="text-muted" style={{ fontSize: '0.75rem', marginBottom: '0.25rem' }}>Profile</div>
                    <div>{explanation.profile}</div>
                  </div>
                  {explanation.matched_rule && (
                    <div>
                      <div className="text-muted" style={{ fontSize: '0.75rem', marginBottom: '0.25rem' }}>Matched Rule</div>
                      <div>{explanation.matched_rule}</div>
                    </div>
                  )}
                  <div>
                    <div className="text-muted" style={{ fontSize: '0.75rem', marginBottom: '0.25rem' }}>Score</div>
                    <div>weight={explanation.selected.score.weight} penalty={explanation.selected.score.health_penalty}</div>
                  </div>
                </div>
              ) : (
                <p className="text-muted">No provider could serve this request. Check rejections below.</p>
              )}
            </div>
          </div>

          {/* Model Resolution */}
          {explanation.model_resolution.length > 0 && (
            <div className="card" style={{ marginBottom: '1.5rem' }}>
              <div className="card-header">
                <h3>Model Resolution</h3>
              </div>
              <div className="card-body">
                <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
                  {explanation.model_resolution.map((step, i) => (
                    <div key={i} style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
                      <span className="type-badge">{step.step}</span>
                      {step.from && step.to && (
                        <>
                          <span className="text-mono" style={{ fontSize: '0.85rem' }}>{step.from}</span>
                          <ArrowRight size={14} />
                          <span className="text-mono" style={{ fontSize: '0.85rem' }}>{step.to}</span>
                        </>
                      )}
                      {step.model && <span className="text-mono" style={{ fontSize: '0.85rem' }}>{step.model}</span>}
                      {step.rule && <span className="text-muted" style={{ fontSize: '0.8rem' }}>({step.rule})</span>}
                    </div>
                  ))}
                </div>
              </div>
            </div>
          )}

          {/* Scoring */}
          {explanation.scoring.length > 0 && (
            <div className="card" style={{ marginBottom: '1.5rem' }}>
              <div className="card-header">
                <h3>Candidate Scoring</h3>
              </div>
              <div className="table-wrapper">
                <table className="table">
                  <thead>
                    <tr>
                      <th>Rank</th>
                      <th>Candidate</th>
                      <th style={{ textAlign: 'right' }}>Weight</th>
                      <th style={{ textAlign: 'right' }}>Latency (ms)</th>
                      <th style={{ textAlign: 'right' }}>Inflight</th>
                      <th style={{ textAlign: 'right' }}>Health Penalty</th>
                    </tr>
                  </thead>
                  <tbody>
                    {explanation.scoring.map((entry) => (
                      <tr key={entry.candidate} className={entry.rank === 1 ? 'table-row--highlight' : ''}>
                        <td className="text-bold">{entry.rank}</td>
                        <td className="text-mono" style={{ fontSize: '0.85rem' }}>{entry.candidate}</td>
                        <td style={{ textAlign: 'right' }}>{entry.score.weight}</td>
                        <td style={{ textAlign: 'right' }}>{entry.score.latency_ms ?? '-'}</td>
                        <td style={{ textAlign: 'right' }}>{entry.score.inflight ?? '-'}</td>
                        <td style={{ textAlign: 'right' }}>{entry.score.health_penalty}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}

          {/* Alternates */}
          {explanation.alternates.length > 0 && (
            <div className="card" style={{ marginBottom: '1.5rem' }}>
              <div className="card-header">
                <h3>Alternate Routes</h3>
              </div>
              <div className="table-wrapper">
                <table className="table">
                  <thead>
                    <tr>
                      <th>Provider</th>
                      <th>Credential</th>
                      <th>Model</th>
                      <th style={{ textAlign: 'right' }}>Weight</th>
                    </tr>
                  </thead>
                  <tbody>
                    {explanation.alternates.map((alt, i) => (
                      <tr key={i}>
                        <td className="text-bold">{alt.provider}</td>
                        <td className="text-mono" style={{ fontSize: '0.85rem' }}>{alt.credential_name}</td>
                        <td className="text-mono" style={{ fontSize: '0.85rem' }}>{alt.model}</td>
                        <td style={{ textAlign: 'right' }}>{alt.score.weight}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}

          {/* Rejections */}
          {explanation.rejections.length > 0 && (
            <div className="card">
              <div className="card-header">
                <h3><XCircle size={18} color="var(--color-danger)" style={{ verticalAlign: 'middle', marginRight: '0.5rem' }} />Rejections</h3>
              </div>
              <div className="table-wrapper">
                <table className="table">
                  <thead>
                    <tr>
                      <th>Candidate</th>
                      <th>Reason</th>
                    </tr>
                  </thead>
                  <tbody>
                    {explanation.rejections.map((rej, i) => (
                      <tr key={i}>
                        <td className="text-mono" style={{ fontSize: '0.85rem' }}>{rej.candidate}</td>
                        <td>{formatRejectReason(rej.reason)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
}
