import { useCallback, useEffect, useMemo, useState } from 'react';
import { providersApi } from '../services/api';
import type { ProbeStatus, ProviderCapabilityEntry } from '../types';
import {
  CheckCircle,
  Filter,
  Layers,
  RefreshCw,
  Search,
  XCircle,
} from 'lucide-react';

interface ModelEntry {
  id: string;
  alias: string | null;
  provider: string;
  upstream: string;
  format: string;
  wire_api: string;
}

function ProbeBadge({ state }: { state: ProbeStatus }) {
  if (state === 'verified') {
    return (
      <span className="type-badge type-badge--green">
        <CheckCircle size={12} />
        Verified
      </span>
    );
  }
  if (state === 'failed') {
    return (
      <span className="type-badge type-badge--red">
        <XCircle size={12} />
        Failed
      </span>
    );
  }
  if (state === 'unsupported') {
    return <span className="type-badge">Unsupported</span>;
  }
  return <span className="type-badge">Unknown</span>;
}

function HealthBadge({
  status,
  checkedAt,
}: {
  status: ProviderCapabilityEntry['probe_status'];
  checkedAt?: string | null;
}) {
  const title = checkedAt ? `Last checked ${new Date(checkedAt).toLocaleString()}` : undefined;
  if (status === 'ok') {
    return (
      <span className="type-badge type-badge--green" title={title}>
        Ready
      </span>
    );
  }
  if (status === 'error') {
    return (
      <span className="type-badge type-badge--red" title={title}>
        Error
      </span>
    );
  }
  if (status === 'warning') {
    return (
      <span className="type-badge type-badge--blue" title={title}>
        Partial
      </span>
    );
  }
  return (
    <span className="type-badge" title={title}>
      Unknown
    </span>
  );
}

function presentationLabel(profile: ProviderCapabilityEntry['presentation_profile']) {
  switch (profile) {
    case 'claude-code':
      return 'Claude Code';
    case 'gemini-cli':
      return 'Gemini CLI';
    case 'codex-cli':
      return 'Codex CLI';
    default:
      return 'Native';
  }
}

function protocolLabel(protocol: ProviderCapabilityEntry['upstream_protocol']) {
  switch (protocol) {
    case 'open_ai':
      return 'OpenAI';
    case 'claude':
    case 'anthropic':
      return 'Anthropic';
    case 'gemini':
      return 'Gemini';
    default:
      return protocol;
  }
}

export default function ModelsCapabilities() {
  const [providers, setProviders] = useState<ProviderCapabilityEntry[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState('');
  const [filterUpstream, setFilterUpstream] = useState('');
  const [filterProvider, setFilterProvider] = useState('');

  const fetchCapabilities = useCallback(async () => {
    try {
      const next = await providersApi.capabilities();
      setProviders(next);
    } catch (err) {
      console.error('Failed to fetch provider capabilities:', err);
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchCapabilities();
  }, [fetchCapabilities]);

  const activeProviders = useMemo(
    () => providers.filter((provider) => !provider.disabled),
    [providers],
  );

  const allModels = useMemo(() => {
    const models: ModelEntry[] = [];
    for (const provider of activeProviders) {
      for (const model of provider.models) {
        models.push({
          id: model.id,
          alias: model.alias ?? null,
          provider: provider.name,
          upstream: provider.upstream,
          format: provider.format,
          wire_api: provider.wire_api,
        });
      }
    }
    return models;
  }, [activeProviders]);

  const filteredModels = useMemo(() => {
    let result = allModels;
    if (searchQuery) {
      const q = searchQuery.toLowerCase();
      result = result.filter(
        (model) =>
          model.id.toLowerCase().includes(q) ||
          (model.alias && model.alias.toLowerCase().includes(q)) ||
          model.provider.toLowerCase().includes(q) ||
          model.upstream.toLowerCase().includes(q),
      );
    }
    if (filterUpstream) {
      result = result.filter((model) => model.upstream === filterUpstream);
    }
    if (filterProvider) {
      result = result.filter((model) => model.provider === filterProvider);
    }
    return result;
  }, [allModels, filterProvider, filterUpstream, searchQuery]);

  const groupedModels = useMemo(() => {
    const groups = new Map<string, ModelEntry[]>();
    for (const model of filteredModels) {
      const entries = groups.get(model.id) ?? [];
      entries.push(model);
      groups.set(model.id, entries);
    }
    return [...groups.entries()].sort(([left], [right]) => left.localeCompare(right));
  }, [filteredModels]);

  const providerNames = useMemo(
    () => activeProviders.map((provider) => provider.name),
    [activeProviders],
  );
  const upstreams = useMemo(
    () => [...new Set(activeProviders.map((provider) => provider.upstream))],
    [activeProviders],
  );

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h2>Models & Capabilities</h2>
          <p className="page-subtitle">
            {allModels.length} provider-model mappings across {activeProviders.length} active providers
          </p>
        </div>
        <div className="page-header-actions">
          <button className="btn btn-secondary" onClick={fetchCapabilities}>
            <RefreshCw size={16} />
            Refresh
          </button>
        </div>
      </div>

      <div className="card" style={{ marginBottom: '1.5rem' }}>
        <div className="card-header">
          <h3>Provider Runtime Truth</h3>
        </div>
        <div className="card-body">
          {isLoading ? (
            <div className="empty-state"><p>Loading runtime capability truth...</p></div>
          ) : activeProviders.length === 0 ? (
            <div className="empty-state">
              <Layers size={48} />
              <p>No active providers configured</p>
            </div>
          ) : (
            <>
              <p className="text-muted" style={{ marginBottom: '1rem' }}>
                This view is driven from the dashboard control-plane payload instead of joining provider config
                with guessed defaults in the browser. Unknown means the provider exists but no successful live
                probe has been recorded for that capability yet.
              </p>
              <div className="table-wrapper">
                <table className="table">
                  <thead>
                    <tr>
                      <th>Provider</th>
                      <th>Identity</th>
                      <th style={{ textAlign: 'center' }}>Wire API</th>
                      <th style={{ textAlign: 'center' }}>Models</th>
                      <th style={{ textAlign: 'center' }}>Health</th>
                      <th style={{ textAlign: 'center' }}>Text</th>
                      <th style={{ textAlign: 'center' }}>Streaming</th>
                      <th style={{ textAlign: 'center' }}>Tools</th>
                      <th style={{ textAlign: 'center' }}>Images</th>
                      <th style={{ textAlign: 'center' }}>JSON Schema</th>
                      <th style={{ textAlign: 'center' }}>Reasoning</th>
                      <th style={{ textAlign: 'center' }}>Count Tokens</th>
                      <th style={{ textAlign: 'center' }}>Presentation</th>
                    </tr>
                  </thead>
                  <tbody>
                    {activeProviders.map((provider) => (
                      <tr key={provider.name}>
                        <td className="text-bold">{provider.name}</td>
                        <td>
                          <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
                            <span className="type-badge">{provider.format}</span>
                            <span className="type-badge" style={{ opacity: 0.85 }}>{provider.upstream}</span>
                            <span className="type-badge" style={{ opacity: 0.7 }}>{protocolLabel(provider.upstream_protocol)}</span>
                          </div>
                        </td>
                        <td style={{ textAlign: 'center' }}>
                          <span className="type-badge">{provider.wire_api}</span>
                        </td>
                        <td style={{ textAlign: 'center' }}>{provider.models.length}</td>
                        <td style={{ textAlign: 'center' }}>
                          <HealthBadge status={provider.probe_status} checkedAt={provider.checked_at} />
                        </td>
                        <td style={{ textAlign: 'center' }}>
                          <ProbeBadge state={provider.probe.text.status} />
                        </td>
                        <td style={{ textAlign: 'center' }}>
                          <ProbeBadge state={provider.probe.stream.status} />
                        </td>
                        <td style={{ textAlign: 'center' }}>
                          <ProbeBadge state={provider.probe.tools.status} />
                        </td>
                        <td style={{ textAlign: 'center' }}>
                          <ProbeBadge state={provider.probe.images.status} />
                        </td>
                        <td style={{ textAlign: 'center' }}>
                          <ProbeBadge state={provider.probe.json_schema.status} />
                        </td>
                        <td style={{ textAlign: 'center' }}>
                          <ProbeBadge state={provider.probe.reasoning.status} />
                        </td>
                        <td style={{ textAlign: 'center' }}>
                          <ProbeBadge state={provider.probe.count_tokens.status} />
                        </td>
                        <td style={{ textAlign: 'center' }}>
                          <span className="type-badge">
                            {presentationLabel(provider.presentation_profile)}
                          </span>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </>
          )}
        </div>
      </div>

      <div className="card">
        <div className="card-header card-header--actions">
          <h3>Model Registry</h3>
          <div style={{ display: 'flex', gap: '0.5rem', alignItems: 'center' }}>
            <Filter size={14} className="text-muted" />
            <select
              className="filter-input"
              value={filterUpstream}
              onChange={(event) => setFilterUpstream(event.target.value)}
              style={{ minWidth: 120 }}
            >
              <option value="">All Upstreams</option>
              {upstreams.map((upstream) => (
                <option key={upstream} value={upstream}>{upstream}</option>
              ))}
            </select>
            <select
              className="filter-input"
              value={filterProvider}
              onChange={(event) => setFilterProvider(event.target.value)}
              style={{ minWidth: 140 }}
            >
              <option value="">All Providers</option>
              {providerNames.map((name) => (
                <option key={name} value={name}>{name}</option>
              ))}
            </select>
            <div className="search-input-wrapper">
              <Search size={14} className="search-icon" />
              <input
                type="text"
                placeholder="Search models, aliases, providers..."
                className="filter-input search-input"
                value={searchQuery}
                onChange={(event) => setSearchQuery(event.target.value)}
              />
            </div>
          </div>
        </div>
        <div className="card-body" style={{ padding: 0 }}>
          {isLoading ? (
            <div className="empty-state" style={{ padding: '2rem' }}><p>Loading...</p></div>
          ) : groupedModels.length === 0 ? (
            <div className="empty-state" style={{ padding: '2rem' }}>
              <Layers size={48} />
              <p>No models match the current filters</p>
            </div>
          ) : (
            <div className="table-wrapper">
              <table className="table">
                <thead>
                  <tr>
                    <th>Model ID</th>
                    <th>Aliases</th>
                    <th>Available From</th>
                    <th style={{ textAlign: 'center' }}>Providers</th>
                  </tr>
                </thead>
                <tbody>
                  {groupedModels.map(([modelId, entries]) => {
                    const aliases = [...new Set(entries.map((entry) => entry.alias).filter(Boolean))] as string[];
                    return (
                      <tr key={modelId}>
                        <td className="text-mono text-bold" style={{ fontSize: '0.85rem' }}>
                          {modelId}
                        </td>
                        <td className="text-mono" style={{ fontSize: '0.85rem' }}>
                          {aliases.length > 0 ? aliases.join(', ') : <span className="text-muted">-</span>}
                        </td>
                        <td>
                          <div style={{ display: 'flex', gap: '0.25rem', flexWrap: 'wrap' }}>
                            {entries.map((entry) => (
                              <span key={`${entry.provider}-${entry.id}-${entry.alias ?? 'native'}`} className="type-badge">
                                {entry.provider} · {entry.upstream}
                              </span>
                            ))}
                          </div>
                        </td>
                        <td style={{ textAlign: 'center' }}>{entries.length}</td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )}
        </div>
      </div>

      {!isLoading && (
        <div className="text-muted" style={{ marginTop: '1rem', fontSize: '0.8rem' }}>
          Showing {filteredModels.length} provider-model mappings across {groupedModels.length} unique model IDs.
        </div>
      )}
    </div>
  );
}
