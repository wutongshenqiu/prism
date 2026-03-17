import { Panel } from '../Panel';
import { StatusPill } from '../StatusPill';
import type { ProviderAtlasResponse, ProviderAtlasRow } from '../../types/controlPlane';
import type { ProtocolCoverageEntry, ProviderCapabilityEntry } from '../../types/backend';
import type { ProviderAtlasModelInventoryItem, ProviderAtlasProtocolFacts } from './types';

function protocolCoverageLabel(mode?: string | null) {
  if (!mode) return 'unsupported';
  if (mode === 'native') return 'native';
  return 'adapted';
}

interface ProviderAtlasOverviewProps {
  loading: boolean;
  error: string | null;
  data: ProviderAtlasResponse | null;
  selectedProvider: string | null;
  selectedRow: ProviderAtlasRow | null;
  selectedCapabilities: ProviderCapabilityEntry | null;
  protocolFacts: ProviderAtlasProtocolFacts;
  filteredProtocolCoverage: ProtocolCoverageEntry[];
  filteredModelInventory: ProviderAtlasModelInventoryItem[];
  protocolSearch: string;
  modelSearch: string;
  onSelectProvider: (provider: string) => void;
  onProtocolSearchChange: (value: string) => void;
  onModelSearchChange: (value: string) => void;
  onOpenRegistryWorkbench: () => void;
}

export function ProviderAtlasOverview({
  loading,
  error,
  data,
  selectedProvider,
  selectedRow,
  selectedCapabilities,
  protocolFacts,
  filteredProtocolCoverage,
  filteredModelInventory,
  protocolSearch,
  modelSearch,
  onSelectProvider,
  onProtocolSearchChange,
  onModelSearchChange,
  onOpenRegistryWorkbench,
}: ProviderAtlasOverviewProps) {
  return (
    <>
      {selectedRow ? (
        <div className="status-message status-message--warning">
          Active provider: <strong>{selectedRow.provider}</strong> · {selectedRow.status} · {selectedRow.auth}
        </div>
      ) : null}

      <div className="two-column">
        <Panel title="Provider roster" subtitle="Entity graph for providers, auth profiles, and live probe posture." className="panel--wide">
          <div className="inline-actions">
            <button type="button" className="button button--ghost" onClick={onOpenRegistryWorkbench}>
              Provider registry
            </button>
          </div>
          <div className="table-grid table-grid--providers">
            <div className="table-grid__head">Provider</div>
            <div className="table-grid__head">Format</div>
            <div className="table-grid__head">Auth</div>
            <div className="table-grid__head">Status</div>
            <div className="table-grid__head">Rotation</div>
            {loading && !data ? <div className="table-grid__cell">Loading providers…</div> : null}
            {error && !data ? <div className="table-grid__cell">{error}</div> : null}
            {(data?.providers ?? []).flatMap((provider) => {
              const selected = provider.provider === selectedProvider;
              const cellClass = `table-grid__cell ${selected ? 'is-selected' : ''} is-clickable`;
              return [
                <div
                  key={`${provider.provider}-name`}
                  className={`${cellClass} table-grid__cell--strong`}
                  onClick={() => onSelectProvider(provider.provider)}
                >
                  {provider.provider}
                </div>,
                <div key={`${provider.provider}-format`} className={cellClass} onClick={() => onSelectProvider(provider.provider)}>
                  {provider.format}
                </div>,
                <div key={`${provider.provider}-auth`} className={cellClass} onClick={() => onSelectProvider(provider.provider)}>
                  {provider.auth}
                </div>,
                <div key={`${provider.provider}-status`} className={cellClass} onClick={() => onSelectProvider(provider.provider)}>
                  <StatusPill label={provider.status} tone={provider.status_tone} />
                </div>,
                <div key={`${provider.provider}-rotation`} className={cellClass} onClick={() => onSelectProvider(provider.provider)}>
                  {provider.rotation}
                </div>,
              ];
            })}
          </div>
        </Panel>

        <Panel title="Capability coverage" subtitle="Protocol truth, model surface, and auth/runtime readiness.">
          <ul className="fact-list">
            {(data?.coverage ?? []).map((fact) => (
              <li key={fact.label}><span>{fact.label}</span><strong>{fact.value}</strong></li>
            ))}
            {selectedCapabilities ? (
              <>
                <li><span>Probe status</span><strong>{selectedCapabilities.probe_status}</strong></li>
                <li><span>Presentation</span><strong>{selectedCapabilities.presentation_profile}</strong></li>
                <li><span>Model surface</span><strong>{selectedCapabilities.models.length}</strong></li>
                <li><span>Tool support</span><strong>{selectedCapabilities.probe.tools.status}</strong></li>
              </>
            ) : null}
          </ul>
        </Panel>
      </div>

      <div className="two-column">
        <Panel title="Protocol surfaces" subtitle="Ingress routes, execution modes, and provider truth should be visible without opening legacy pages.">
          <div className="inline-actions">
            <input
              name="provider-protocol-search"
              placeholder="Filter protocol surfaces"
              autoComplete="off"
              value={protocolSearch}
              onChange={(event) => onProtocolSearchChange(event.target.value)}
            />
          </div>
          <ul className="fact-list">
            <li><span>Public routes</span><strong>{protocolFacts.publicRoutes}</strong></li>
            <li><span>Provider routes</span><strong>{protocolFacts.providerRoutes}</strong></li>
            <li><span>Native surfaces</span><strong>{protocolFacts.nativeSurfaces}</strong></li>
            <li><span>Adapted surfaces</span><strong>{protocolFacts.adaptedSurfaces}</strong></li>
          </ul>
          <div className="probe-list">
            {filteredProtocolCoverage.map((entry) => (
              <div key={`${entry.provider}-${entry.surface_id}`} className="probe-check">
                <span>{entry.surface_label}</span>
                <strong>{protocolCoverageLabel(entry.execution_mode)}</strong>
              </div>
            ))}
          </div>
        </Panel>

        <Panel title="Model inventory" subtitle="Unique model mappings and runtime capability truth are part of provider operations, not a separate admin silo.">
          <div className="inline-actions">
            <input
              name="provider-model-search"
              placeholder="Filter model inventory"
              autoComplete="off"
              value={modelSearch}
              onChange={(event) => onModelSearchChange(event.target.value)}
            />
          </div>
          {filteredModelInventory.length === 0 ? (
            <div className="status-message">No provider model inventory is configured yet.</div>
          ) : (
            <div className="probe-list">
              {filteredModelInventory.map((item) => (
                <div key={`${item.provider}-${item.id}`} className="probe-check">
                  <span>{item.id}</span>
                  <strong>{item.provider} · {item.probe}</strong>
                </div>
              ))}
            </div>
          )}
        </Panel>
      </div>
    </>
  );
}
