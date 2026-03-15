import { useEffect, useState, useCallback, useMemo } from 'react';
import { providersApi } from '../services/api';
import type { Provider, ProviderCapabilityEntry } from '../types';
import {
  Layers,
  RefreshCw,
  Search,
  CheckCircle,
  XCircle,
  Filter,
} from 'lucide-react';

interface ModelEntry {
  id: string;
  alias: string | null;
  provider: string;
  format: string;
}

export default function ModelsCapabilities() {
  const [providers, setProviders] = useState<Provider[]>([]);
  const [capabilityMap, setCapabilityMap] = useState<Record<string, ProviderCapabilityEntry>>({});
  const [isLoading, setIsLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState('');
  const [filterFormat, setFilterFormat] = useState('');
  const [filterProvider, setFilterProvider] = useState('');

  const fetchProviders = useCallback(async () => {
    try {
      const [provRes, caps] = await Promise.all([
        providersApi.list(),
        providersApi.capabilities().catch(() => [] as ProviderCapabilityEntry[]),
      ]);
      setProviders(provRes.data);
      const map: Record<string, ProviderCapabilityEntry> = {};
      for (const c of caps) {
        map[c.name] = c;
      }
      setCapabilityMap(map);
    } catch (err) {
      console.error('Failed to fetch providers:', err);
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchProviders();
  }, [fetchProviders]);

  const allModels = useMemo(() => {
    const models: ModelEntry[] = [];
    for (const p of providers) {
      if (p.disabled) continue;
      for (const m of p.models) {
        models.push({
          id: m.id,
          alias: m.alias,
          provider: p.name,
          format: p.format,
        });
      }
    }
    return models;
  }, [providers]);

  const filteredModels = useMemo(() => {
    let result = allModels;
    if (searchQuery) {
      const q = searchQuery.toLowerCase();
      result = result.filter(
        (m) =>
          m.id.toLowerCase().includes(q) ||
          (m.alias && m.alias.toLowerCase().includes(q)) ||
          m.provider.toLowerCase().includes(q)
      );
    }
    if (filterFormat) {
      result = result.filter((m) => m.format === filterFormat);
    }
    if (filterProvider) {
      result = result.filter((m) => m.provider === filterProvider);
    }
    return result;
  }, [allModels, searchQuery, filterFormat, filterProvider]);

  // Group models by model ID to show multi-provider availability
  const modelGroups = useMemo(() => {
    const groups = new Map<string, ModelEntry[]>();
    for (const m of filteredModels) {
      const key = m.alias || m.id;
      const arr = groups.get(key) || [];
      arr.push(m);
      groups.set(key, arr);
    }
    return [...groups.entries()].sort(([a], [b]) => a.localeCompare(b));
  }, [filteredModels]);

  const uniqueFormats = [...new Set(providers.map((p) => p.format))];
  const activeProviders = providers.filter((p) => !p.disabled);
  const providerNames = activeProviders.map((p) => p.name);

  // Capability summary per provider — sourced from capabilities API
  const providerCapabilities = useMemo(() => {
    return activeProviders.map((p) => {
      const caps = capabilityMap[p.name]?.capabilities;
      return {
        name: p.name,
        format: p.format,
        modelsCount: p.models.length,
        supportsStream: caps?.supports_stream ?? true,
        supportsTools: caps?.supports_tools ?? false,
        wireApi: p.wire_api,
        hasPresentation: !!p.upstream_presentation && p.upstream_presentation.profile !== 'native',
      };
    });
  }, [activeProviders, capabilityMap]);

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h2>Models & Capabilities</h2>
          <p className="page-subtitle">
            {allModels.length} models across {activeProviders.length} active providers
          </p>
        </div>
        <div className="page-header-actions">
          <button className="btn btn-secondary" onClick={fetchProviders}>
            <RefreshCw size={16} />
            Refresh
          </button>
        </div>
      </div>

      {/* Provider Capability Matrix */}
      <div className="card" style={{ marginBottom: '1.5rem' }}>
        <div className="card-header">
          <h3>Provider Capabilities</h3>
        </div>
        <div className="card-body">
          {isLoading ? (
            <div className="empty-state"><p>Loading...</p></div>
          ) : providerCapabilities.length === 0 ? (
            <div className="empty-state">
              <Layers size={48} />
              <p>No active providers configured</p>
            </div>
          ) : (
            <div className="table-wrapper">
              <table className="table">
                <thead>
                  <tr>
                    <th>Provider</th>
                    <th>Format</th>
                    <th style={{ textAlign: 'center' }}>Models</th>
                    <th style={{ textAlign: 'center' }}>Wire API</th>
                    <th style={{ textAlign: 'center' }}>Streaming</th>
                    <th style={{ textAlign: 'center' }}>Tools</th>
                    <th style={{ textAlign: 'center' }}>Presentation</th>
                  </tr>
                </thead>
                <tbody>
                  {providerCapabilities.map((cap) => (
                    <tr key={cap.name}>
                      <td className="text-bold">{cap.name}</td>
                      <td><span className="type-badge">{cap.format}</span></td>
                      <td style={{ textAlign: 'center' }}>{cap.modelsCount}</td>
                      <td style={{ textAlign: 'center' }}>
                        <span className="type-badge">{cap.wireApi}</span>
                      </td>
                      <td style={{ textAlign: 'center' }}>
                        <CheckCircle size={14} color="var(--color-success)" />
                      </td>
                      <td style={{ textAlign: 'center' }}>
                        {cap.supportsTools
                          ? <CheckCircle size={14} color="var(--color-success)" />
                          : <XCircle size={14} color="var(--color-text-secondary)" />
                        }
                      </td>
                      <td style={{ textAlign: 'center' }}>
                        {cap.hasPresentation
                          ? <CheckCircle size={14} color="var(--color-success)" />
                          : <span className="text-muted">-</span>
                        }
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      </div>

      {/* Model Search & Filter */}
      <div className="card">
        <div className="card-header card-header--actions">
          <h3>Model Registry</h3>
          <div style={{ display: 'flex', gap: '0.5rem', alignItems: 'center' }}>
            <Filter size={14} className="text-muted" />
            <select
              className="filter-input"
              value={filterFormat}
              onChange={(e) => setFilterFormat(e.target.value)}
              style={{ minWidth: 100 }}
            >
              <option value="">All Formats</option>
              {uniqueFormats.map((f) => (
                <option key={f} value={f}>{f}</option>
              ))}
            </select>
            <select
              className="filter-input"
              value={filterProvider}
              onChange={(e) => setFilterProvider(e.target.value)}
              style={{ minWidth: 120 }}
            >
              <option value="">All Providers</option>
              {providerNames.map((n) => (
                <option key={n} value={n}>{n}</option>
              ))}
            </select>
            <div className="search-input-wrapper">
              <Search size={14} className="search-icon" />
              <input
                type="text"
                placeholder="Search models..."
                className="filter-input search-input"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
              />
            </div>
          </div>
        </div>
        <div className="card-body" style={{ padding: 0 }}>
          {isLoading ? (
            <div className="empty-state" style={{ padding: '2rem' }}><p>Loading...</p></div>
          ) : modelGroups.length === 0 ? (
            <div className="empty-state" style={{ padding: '2rem' }}>
              <Layers size={48} />
              <p>No models match your search</p>
            </div>
          ) : (
            <div className="table-wrapper">
              <table className="table">
                <thead>
                  <tr>
                    <th>Model</th>
                    <th>Alias</th>
                    <th>Available From</th>
                    <th style={{ textAlign: 'center' }}>Providers</th>
                  </tr>
                </thead>
                <tbody>
                  {modelGroups.map(([key, entries]) => (
                    <tr key={key}>
                      <td className="text-mono text-bold" style={{ fontSize: '0.85rem' }}>
                        {entries[0].id}
                      </td>
                      <td className="text-mono" style={{ fontSize: '0.85rem' }}>
                        {entries[0].alias && entries[0].alias !== entries[0].id ? entries[0].alias : <span className="text-muted">-</span>}
                      </td>
                      <td>
                        <div style={{ display: 'flex', gap: '0.25rem', flexWrap: 'wrap' }}>
                          {entries.map((e) => (
                            <span key={`${e.provider}-${e.id}`} className="type-badge">
                              {e.provider}
                            </span>
                          ))}
                        </div>
                      </td>
                      <td style={{ textAlign: 'center' }}>{entries.length}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      </div>

      {/* Summary stats */}
      {!isLoading && (
        <div className="text-muted" style={{ marginTop: '1rem', fontSize: '0.8rem' }}>
          Showing {filteredModels.length} of {allModels.length} models ({modelGroups.length} unique)
        </div>
      )}
    </div>
  );
}
