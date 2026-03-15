import { useCallback, useEffect, useMemo, useState } from 'react';
import { protocolsApi } from '../services/api';
import type {
  ProbeStatus,
  ProtocolCoverageEntry,
  ProtocolEndpointEntry,
} from '../types';
import {
  ArrowRight,
  CheckCircle,
  Network,
  RefreshCw,
  XCircle,
} from 'lucide-react';

interface ProtocolMatrixState {
  endpoints: ProtocolEndpointEntry[];
  coverage: ProtocolCoverageEntry[];
}

const FAMILY_LABELS: Record<ProtocolEndpointEntry['family'], string> = {
  open_ai: 'OpenAI',
  claude: 'Claude',
  gemini: 'Gemini',
};

const STREAM_LABELS: Record<ProtocolEndpointEntry['stream_transport'], string> = {
  none: 'No',
  sse: 'SSE',
  web_socket_events: 'WebSocket events',
};

const SCOPE_LABELS: Record<ProtocolEndpointEntry['scope'], string> = {
  public: 'Public',
  provider_scoped: 'Provider-scoped',
};

type CoverageLevel = 'native' | 'adapted' | 'unsupported';

function routeOrder(path: string): number {
  const order = [
    '/v1/chat/completions',
    '/v1/completions',
    '/v1/responses',
    '/v1/responses/ws',
    '/v1/models',
    '/v1/messages',
    '/v1/messages/count_tokens',
    '/v1beta/models/{model}:generateContent',
    '/v1beta/models/{model}:streamGenerateContent',
    '/v1beta/models',
    '/api/provider/{provider}/v1/chat/completions',
    '/api/provider/{provider}/v1/messages',
    '/api/provider/{provider}/v1/responses',
    '/api/provider/{provider}/v1/responses/ws',
  ];
  const index = order.indexOf(path);
  return index === -1 ? order.length : index;
}

function executionModeToCoverage(mode?: ProtocolCoverageEntry['execution_mode']): CoverageLevel {
  if (mode === 'native') return 'native';
  if (mode) return 'adapted';
  return 'unsupported';
}

function StateBadge({
  state,
}: {
  state: {
    status: ProbeStatus;
    message?: string | null;
  };
}) {
  if (state.status === 'verified') {
    return (
      <span className="type-badge type-badge--green" title={state.message ?? undefined}>
        <CheckCircle size={12} />
        Verified
      </span>
    );
  }
  if (state.status === 'failed') {
    return (
      <span className="type-badge type-badge--red" title={state.message ?? undefined}>
        <XCircle size={12} />
        Failed
      </span>
    );
  }
  if (state.status === 'unsupported') {
    return (
      <span className="type-badge" title={state.message ?? undefined}>
        Unsupported
      </span>
    );
  }
  return (
    <span className="type-badge" title={state.message ?? undefined}>
      Unknown
    </span>
  );
}

function CoverageCell({ entry }: { entry?: ProtocolCoverageEntry }) {
  if (!entry) {
    return <span className="type-badge">Not exposed</span>;
  }

  if (entry.state.status === 'unsupported' || !entry.execution_mode) {
    return (
      <span className="type-badge" title={entry.state.message ?? undefined}>
        Unsupported
      </span>
    );
  }

  const label = executionModeToCoverage(entry.execution_mode) === 'native' ? 'Native' : 'Adapted';
  if (entry.state.status === 'verified') {
    return (
      <span className="type-badge type-badge--green" title={entry.state.message ?? undefined}>
        {label}
      </span>
    );
  }
  if (entry.state.status === 'failed') {
    return (
      <span className="type-badge type-badge--red" title={entry.state.message ?? undefined}>
        {label} failed
      </span>
    );
  }
  return (
    <span className="type-badge type-badge--blue" title={entry.state.message ?? undefined}>
      {label} unknown
    </span>
  );
}

export default function Protocols() {
  const [matrix, setMatrix] = useState<ProtocolMatrixState>({
    endpoints: [],
    coverage: [],
  });
  const [isLoading, setIsLoading] = useState(true);

  const fetchMatrix = useCallback(async () => {
    try {
      const next = await protocolsApi.matrix();
      setMatrix(next);
    } catch (err) {
      console.error('Failed to fetch protocol matrix:', err);
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchMatrix();
  }, [fetchMatrix]);

  const endpointsByScope = useMemo(() => {
    const sorted = [...matrix.endpoints].sort((a, b) => routeOrder(a.path) - routeOrder(b.path));
    return {
      public: sorted.filter((entry) => entry.scope === 'public'),
      providerScoped: sorted.filter((entry) => entry.scope === 'provider_scoped'),
    };
  }, [matrix.endpoints]);

  const activeCoverage = useMemo(
    () => matrix.coverage.filter((entry) => !entry.disabled),
    [matrix.coverage],
  );

  const surfaces = useMemo(() => {
    const seen = new Set<string>();
    return activeCoverage.filter((entry) => {
      if (seen.has(entry.surface_id)) return false;
      seen.add(entry.surface_id);
      return true;
    });
  }, [activeCoverage]);

  const providers = useMemo(() => {
    const byProvider = new Map<string, { meta: ProtocolCoverageEntry; cells: Map<string, ProtocolCoverageEntry> }>();
    for (const entry of activeCoverage) {
      const existing = byProvider.get(entry.provider);
      if (existing) {
        existing.cells.set(entry.surface_id, entry);
        continue;
      }
      byProvider.set(entry.provider, {
        meta: entry,
        cells: new Map([[entry.surface_id, entry]]),
      });
    }
    return [...byProvider.values()];
  }, [activeCoverage]);

  const hiddenDisabledProviders = new Set(
    matrix.coverage.filter((entry) => entry.disabled).map((entry) => entry.provider),
  ).size;

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h2>Protocols</h2>
          <p className="page-subtitle">
            Runtime route inventory and provider surface coverage
          </p>
        </div>
        <div className="page-header-actions">
          <button className="btn btn-secondary" onClick={fetchMatrix}>
            <RefreshCw size={16} />
            Refresh
          </button>
        </div>
      </div>

      <div className="card" style={{ marginBottom: '1.5rem' }}>
        <div className="card-header">
          <h3>
            <Network size={18} style={{ verticalAlign: 'middle', marginRight: '0.5rem' }} />
            Route Inventory
          </h3>
        </div>
        <div className="card-body">
          <p className="text-muted" style={{ marginBottom: '1rem' }}>
            This table is built from the current backend router and cached probe truth. WebSocket routes,
            provider-scoped routes, and non-generation operations are listed explicitly instead of being
            collapsed into three protocol families.
          </p>
          {(['public', 'providerScoped'] as const).map((scopeKey) => {
            const entries = scopeKey === 'public' ? endpointsByScope.public : endpointsByScope.providerScoped;
            return (
              <div key={scopeKey} style={{ marginBottom: scopeKey === 'public' ? '1.5rem' : 0 }}>
                <h4 style={{ marginBottom: '0.75rem' }}>
                  {scopeKey === 'public' ? 'Public Routes' : 'Provider-scoped Routes'}
                </h4>
                <div className="table-wrapper">
                  <table className="table">
                    <thead>
                      <tr>
                        <th>Family</th>
                        <th style={{ width: 80 }}>Method</th>
                        <th>Path</th>
                        <th style={{ width: 120 }}>Transport</th>
                        <th style={{ width: 140 }}>Streaming</th>
                        <th style={{ width: 120 }}>State</th>
                        <th>Description</th>
                      </tr>
                    </thead>
                    <tbody>
                      {entries.map((entry) => (
                        <tr key={entry.id}>
                          <td>
                            <span className="type-badge">{FAMILY_LABELS[entry.family]}</span>
                            <div className="text-muted" style={{ fontSize: '0.75rem', marginTop: 4 }}>
                              {SCOPE_LABELS[entry.scope]}
                            </div>
                          </td>
                          <td>
                            <span className="type-badge">{entry.method}</span>
                          </td>
                          <td className="text-mono" style={{ fontSize: '0.85rem' }}>
                            {entry.path}
                          </td>
                          <td>
                            <span className="type-badge">
                              {entry.transport === 'web_socket' ? 'WebSocket' : 'HTTP'}
                            </span>
                          </td>
                          <td>
                            <span className="type-badge">{STREAM_LABELS[entry.stream_transport]}</span>
                          </td>
                          <td>
                            <StateBadge state={entry.state} />
                          </td>
                          <td>
                            <div>{entry.description}</div>
                            {entry.note && (
                              <div className="text-muted" style={{ fontSize: '0.8rem', marginTop: 4 }}>
                                {entry.note}
                              </div>
                            )}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>
            );
          })}
        </div>
      </div>

      <div className="card">
        <div className="card-header">
          <h3>Provider Surface Coverage</h3>
        </div>
        <div className="card-body">
          {isLoading ? (
            <div className="empty-state"><p>Loading protocol coverage...</p></div>
          ) : providers.length === 0 ? (
            <div className="empty-state">
              <Network size={48} />
              <p>No active providers configured</p>
            </div>
          ) : (
            <>
              <p className="text-muted" style={{ marginBottom: '1rem' }}>
                Coverage is calculated per client surface, not by rough upstream family. For example,
                OpenAI Chat and OpenAI Responses are treated separately because Responses only works with
                OpenAI-format providers, while chat can be adapted into Claude or Gemini.
              </p>
              {hiddenDisabledProviders > 0 && (
                <div className="alert alert-warning" style={{ marginBottom: '1rem' }}>
                  {hiddenDisabledProviders} disabled provider surface{hiddenDisabledProviders === 1 ? '' : 's'} are hidden from this matrix.
                </div>
              )}
              <div className="table-wrapper">
                <table className="table">
                  <thead>
                    <tr>
                      <th>Provider</th>
                      <th>Identity</th>
                      {surfaces.map((surface) => (
                        <th key={surface.surface_id} style={{ textAlign: 'center' }}>
                          {surface.surface_label}
                        </th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {providers.map(({ meta, cells }) => (
                      <tr key={meta.provider}>
                        <td className="text-bold">{meta.provider}</td>
                        <td>
                          <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
                            <span className="type-badge">{meta.format}</span>
                            <span className="type-badge" style={{ opacity: 0.85 }}>{meta.upstream}</span>
                            <span className="type-badge" style={{ opacity: 0.7 }}>{meta.wire_api}</span>
                          </div>
                        </td>
                        {surfaces.map((surface) => (
                          <td key={surface.surface_id} style={{ textAlign: 'center' }}>
                            <CoverageCell entry={cells.get(surface.surface_id)} />
                          </td>
                        ))}
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
              <div className="text-muted" style={{ marginTop: '1rem', fontSize: '0.8rem' }}>
                <ArrowRight size={12} style={{ verticalAlign: 'middle', marginRight: 4 }} />
                Native means the provider speaks that client surface directly. Adapted means Prism translates the request through the canonical gateway pipeline.
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
