import { useEffect, useState, useCallback, useMemo } from 'react';
import { protocolsApi } from '../services/api';
import type { ProtocolMatrixEntry } from '../services/api';
import {
  Network,
  RefreshCw,
  CheckCircle,
  XCircle,
  ArrowRight,
} from 'lucide-react';

interface ProtocolEndpoint {
  method: string;
  path: string;
  description: string;
  streamMode: 'never' | 'runtime';
}

type ProbeState = 'verified' | 'failed' | 'unknown' | 'unsupported';

const PROTOCOLS: {
  id: string;
  label: string;
  format: string;
  endpoints: ProtocolEndpoint[];
}[] = [
  {
    id: 'open_ai',
    label: 'OpenAI',
    format: 'openai',
    endpoints: [
      { method: 'POST', path: '/v1/chat/completions', description: 'Chat completions', streamMode: 'runtime' },
      { method: 'POST', path: '/v1/responses', description: 'Responses API', streamMode: 'runtime' },
      { method: 'GET', path: '/v1/models', description: 'List models', streamMode: 'never' },
    ],
  },
  {
    id: 'claude',
    label: 'Claude (Anthropic)',
    format: 'claude',
    endpoints: [
      { method: 'POST', path: '/v1/messages', description: 'Messages API', streamMode: 'runtime' },
    ],
  },
  {
    id: 'gemini',
    label: 'Gemini (Google)',
    format: 'gemini',
    endpoints: [
      { method: 'POST', path: '/v1beta/models/{model}:generateContent', description: 'Generate content', streamMode: 'never' },
      { method: 'POST', path: '/v1beta/models/{model}:streamGenerateContent', description: 'Stream generate content', streamMode: 'runtime' },
      { method: 'GET', path: '/v1beta/models', description: 'List models', streamMode: 'never' },
    ],
  },
];

type CoverageLevel = 'native' | 'adapted' | 'none';

function executionModeToCoverage(mode: string): CoverageLevel {
  if (mode === 'native') return 'native';
  if (mode === 'lossless_adapted' || mode === 'lossy_adapted') return 'adapted';
  return 'none';
}

function CoverageBadge({ level }: { level: CoverageLevel }) {
  if (level === 'native') {
    return <span className="type-badge type-badge--green"><CheckCircle size={12} /> Native</span>;
  }
  if (level === 'adapted') {
    return <span className="type-badge type-badge--blue"><ArrowRight size={12} /> Adapted</span>;
  }
  return <span className="type-badge type-badge--red"><XCircle size={12} /> None</span>;
}

function StreamSupportBadge({
  mode,
  state,
}: {
  mode: ProtocolEndpoint['streamMode'];
  state: ProbeState;
}) {
  if (mode === 'never') {
    return <span className="text-muted">No</span>;
  }
  if (state === 'verified') {
    return <span className="type-badge type-badge--green"><CheckCircle size={12} /> Verified</span>;
  }
  if (state === 'failed') {
    return <span className="type-badge type-badge--red"><XCircle size={12} /> Failed</span>;
  }
  if (state === 'unsupported') {
    return <span className="type-badge">Unsupported</span>;
  }
  return <span className="type-badge">Unknown</span>;
}

export default function Protocols() {
  const [matrixEntries, setMatrixEntries] = useState<ProtocolMatrixEntry[]>([]);
  const [isLoading, setIsLoading] = useState(true);

  const fetchMatrix = useCallback(async () => {
    try {
      const entries = await protocolsApi.matrix();
      setMatrixEntries(entries);
    } catch (err) {
      console.error('Failed to fetch protocol matrix:', err);
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchMatrix();
  }, [fetchMatrix]);

  // Derive unique providers from the matrix
  const providers = useMemo(() => {
    const seen = new Set<string>();
    return matrixEntries
      .filter((e) => e.supports_generate)
      .filter((e) => { if (seen.has(e.provider)) return false; seen.add(e.provider); return true; })
      .map((e) => ({ name: e.provider, upstream_protocol: e.upstream_protocol }));
  }, [matrixEntries]);

  // Build a coverage lookup: (provider, ingress_protocol) → execution_mode
  const coverageLookup = useMemo(() => {
    const map = new Map<string, string>();
    for (const e of matrixEntries) {
      map.set(`${e.provider}::${e.ingress_protocol}`, e.execution_mode);
    }
    return map;
  }, [matrixEntries]);

  const uniqueFormats = [...new Set(providers.map((p) => p.upstream_protocol))];
  const streamSupportByIngress = useMemo(() => {
    const rank: Record<ProbeState, number> = {
      verified: 4,
      failed: 3,
      unsupported: 2,
      unknown: 1,
    };
    const map = new Map<string, ProbeState>();
    for (const protocol of PROTOCOLS) {
      map.set(protocol.id, 'unknown');
    }
    for (const entry of matrixEntries) {
      if (!entry.supports_generate) continue;
      const current = map.get(entry.ingress_protocol) ?? 'unknown';
      const next = entry.stream_state?.status ?? 'unknown';
      if (rank[next] > rank[current]) {
        map.set(entry.ingress_protocol, next);
      }
    }
    return map;
  }, [matrixEntries]);

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h2>Protocols</h2>
          <p className="page-subtitle">
            Public ingress protocols, endpoint semantics, and provider coverage
          </p>
        </div>
        <div className="page-header-actions">
          <button className="btn btn-secondary" onClick={fetchMatrix}>
            <RefreshCw size={16} />
            Refresh
          </button>
        </div>
      </div>

      {/* Protocol Endpoint Reference */}
      <div className="card" style={{ marginBottom: '1.5rem' }}>
        <div className="card-header">
          <h3><Network size={18} style={{ verticalAlign: 'middle', marginRight: '0.5rem' }} />Public Endpoints</h3>
        </div>
        <div className="card-body">
          <p className="text-muted" style={{ marginBottom: '1rem' }}>
            All public inference endpoints share one canonical runtime pipeline. Requests are parsed by protocol-specific ingress adapters, routed through the capability-aware planner, and translated back via egress adapters. Streaming availability below is derived from the currently active provider set, not hardcoded assumptions.
          </p>
          <div style={{ display: 'flex', flexDirection: 'column', gap: '1.5rem' }}>
            {PROTOCOLS.map((proto) => (
              <div key={proto.id}>
                <h4 style={{ marginBottom: '0.5rem' }}>{proto.label}</h4>
                <div className="table-wrapper">
                  <table className="table">
                    <thead>
                      <tr>
                        <th style={{ width: 80 }}>Method</th>
                        <th>Path</th>
                        <th>Description</th>
                        <th style={{ width: 90 }}>Stream</th>
                      </tr>
                    </thead>
                    <tbody>
                      {proto.endpoints.map((ep) => (
                        <tr key={ep.path}>
                          <td><span className="type-badge">{ep.method}</span></td>
                          <td className="text-mono" style={{ fontSize: '0.85rem' }}>{ep.path}</td>
                          <td>{ep.description}</td>
                          <td>
                            <StreamSupportBadge
                              mode={ep.streamMode}
                              state={streamSupportByIngress.get(proto.id) ?? 'unknown'}
                            />
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* Protocol × Provider Coverage Matrix */}
      <div className="card">
        <div className="card-header">
          <h3>Protocol Coverage Matrix</h3>
        </div>
        <div className="card-body">
          {isLoading ? (
            <div className="empty-state"><p>Loading providers...</p></div>
          ) : providers.length === 0 ? (
            <div className="empty-state">
              <Network size={48} />
              <p>No active providers configured</p>
            </div>
          ) : (
            <>
              <p className="text-muted" style={{ marginBottom: '1rem' }}>
                <strong>Native</strong>: Provider speaks this protocol natively. <strong>Adapted</strong>: Request is translated through the canonical IR to reach this provider.
              </p>
              <div className="table-wrapper">
                <table className="table">
                  <thead>
                    <tr>
                      <th>Provider</th>
                      <th>Protocol</th>
                      {PROTOCOLS.map((p) => (
                        <th key={p.id} style={{ textAlign: 'center' }}>{p.label}</th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {providers.map((provider) => (
                      <tr key={provider.name}>
                        <td className="text-bold">{provider.name}</td>
                        <td><span className="type-badge">{provider.upstream_protocol}</span></td>
                        {PROTOCOLS.map((proto) => {
                          const mode = coverageLookup.get(`${provider.name}::${proto.id}`) || '';
                          return (
                            <td key={proto.id} style={{ textAlign: 'center' }}>
                              <CoverageBadge level={executionModeToCoverage(mode)} />
                            </td>
                          );
                        })}
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </>
          )}
        </div>
      </div>

      {/* Format Distribution */}
      {!isLoading && providers.length > 0 && (
        <div className="card" style={{ marginTop: '1.5rem' }}>
          <div className="card-header">
            <h3>Provider Protocol Distribution</h3>
          </div>
          <div className="card-body">
            <div style={{ display: 'flex', gap: '2rem', flexWrap: 'wrap' }}>
              {uniqueFormats.map((fmt) => {
                const count = providers.filter((p) => p.upstream_protocol === fmt).length;
                return (
                  <div key={fmt} style={{ textAlign: 'center' }}>
                    <div style={{ fontSize: '2rem', fontWeight: 700 }}>{count}</div>
                    <div className="text-muted" style={{ textTransform: 'capitalize' }}>{fmt}</div>
                  </div>
                );
              })}
              <div style={{ textAlign: 'center' }}>
                <div style={{ fontSize: '2rem', fontWeight: 700 }}>{providers.length}</div>
                <div className="text-muted">Total Active</div>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
