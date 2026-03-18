import { Panel } from '../Panel';
import { StatusPill } from '../StatusPill';
import { useI18n } from '../../i18n';
import {
  presentExecutionMode,
  presentFactValue,
  presentPresentationMode,
  presentPresentationProfile,
  presentProbeStatus,
  presentProviderFormat,
} from '../../lib/operatorPresentation';
import type { ProviderAtlasResponse, ProviderAtlasRow } from '../../types/controlPlane';
import type { ProtocolCoverageEntry, ProviderCapabilityEntry } from '../../types/backend';
import type { ProviderAtlasModelInventoryItem, ProviderAtlasProtocolFacts } from './types';

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
  const { t, tx, formatNumber } = useI18n();

  return (
    <>
      {selectedRow ? (
        <div className="status-message status-message--info">
          {t('providerAtlas.status.activeProvider')}{' '}
          <strong>{selectedRow.provider}</strong> · {tx(selectedRow.status)} · {tx(selectedRow.auth)}
        </div>
      ) : null}

      <div className="two-column">
        <Panel
          title={t('providerAtlas.panel.roster.title')}
          subtitle={t('providerAtlas.panel.roster.subtitle')}
          className="panel--wide"
        >
          <div className="inline-actions">
            <button type="button" className="button button--ghost" onClick={onOpenRegistryWorkbench}>
              {t('providerAtlas.panel.roster.registry')}
            </button>
          </div>
          <div className="table-grid table-grid--providers">
            <div className="table-grid__head">{t('common.provider')}</div>
            <div className="table-grid__head">{t('common.format')}</div>
            <div className="table-grid__head">{t('providerAtlas.table.auth')}</div>
            <div className="table-grid__head">{t('common.status')}</div>
            <div className="table-grid__head">{t('providerAtlas.table.rotation')}</div>
            {loading && !data ? <div className="table-grid__cell">{t('providerAtlas.loading.providers')}</div> : null}
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
                  {presentProviderFormat(provider.format)}
                </div>,
                <div key={`${provider.provider}-auth`} className={cellClass} onClick={() => onSelectProvider(provider.provider)}>
                  {tx(provider.auth)}
                </div>,
                <div key={`${provider.provider}-status`} className={cellClass} onClick={() => onSelectProvider(provider.provider)}>
                  <StatusPill label={tx(provider.status)} tone={provider.status_tone} />
                </div>,
                <div key={`${provider.provider}-rotation`} className={cellClass} onClick={() => onSelectProvider(provider.provider)}>
                  {tx(provider.rotation)}
                </div>,
              ];
            })}
          </div>
        </Panel>

        <Panel title={t('providerAtlas.panel.coverage.title')} subtitle={t('providerAtlas.panel.coverage.subtitle')}>
          <ul className="fact-list">
            {(data?.coverage ?? []).map((fact) => (
              <li key={fact.label.key}>
                <span>{tx(fact.label)}</span>
                <strong>{presentFactValue(fact, tx)}</strong>
              </li>
            ))}
            {selectedCapabilities ? (
              <>
                <li><span>{t('providerAtlas.coverage.probeStatus')}</span><strong>{selectedCapabilities.probe_status === 'warning' && !selectedCapabilities.checked_at ? t('providerAtlas.value.probe.notProbed') : presentProbeStatus(selectedCapabilities.probe_status, t)}</strong></li>
                <li><span>{t('providerAtlas.coverage.presentation')}</span><strong>{`${presentPresentationProfile(selectedCapabilities.presentation_profile, t)} / ${presentPresentationMode(selectedCapabilities.presentation_mode, t)}`}</strong></li>
                <li><span>{t('providerAtlas.coverage.modelSurface')}</span><strong>{formatNumber(selectedCapabilities.models.length)}</strong></li>
                <li><span>{t('providerAtlas.coverage.toolSupport')}</span><strong>{presentProbeStatus(selectedCapabilities.probe.tools.status, t)}</strong></li>
              </>
            ) : null}
          </ul>
        </Panel>
      </div>

      <div className="two-column">
        <Panel title={t('providerAtlas.panel.protocol.title')} subtitle={t('providerAtlas.panel.protocol.subtitle')}>
          <div className="inline-actions">
            <input
              name="provider-protocol-search"
              placeholder={t('providerAtlas.panel.protocol.filter')}
              autoComplete="off"
              value={protocolSearch}
              onChange={(event) => onProtocolSearchChange(event.target.value)}
            />
          </div>
          <ul className="fact-list">
            <li><span>{t('providerAtlas.protocol.publicRoutes')}</span><strong>{formatNumber(protocolFacts.publicRoutes)}</strong></li>
            <li><span>{t('providerAtlas.protocol.providerRoutes')}</span><strong>{formatNumber(protocolFacts.providerRoutes)}</strong></li>
            <li><span>{t('providerAtlas.protocol.nativeSurfaces')}</span><strong>{formatNumber(protocolFacts.nativeSurfaces)}</strong></li>
            <li><span>{t('providerAtlas.protocol.adaptedSurfaces')}</span><strong>{formatNumber(protocolFacts.adaptedSurfaces)}</strong></li>
          </ul>
          <div className="probe-list">
            {filteredProtocolCoverage.map((entry) => (
              <div key={`${entry.provider}-${entry.surface_id}`} className="probe-check">
                <span>{entry.surface_label}</span>
                <strong>{presentExecutionMode(entry.execution_mode, t)}</strong>
              </div>
            ))}
          </div>
        </Panel>

        <Panel title={t('providerAtlas.panel.inventory.title')} subtitle={t('providerAtlas.panel.inventory.subtitle')}>
          <div className="inline-actions">
            <input
              name="provider-model-search"
              placeholder={t('providerAtlas.panel.inventory.filter')}
              autoComplete="off"
              value={modelSearch}
              onChange={(event) => onModelSearchChange(event.target.value)}
            />
          </div>
          {filteredModelInventory.length === 0 ? (
            <div className="status-message">{t('providerAtlas.panel.inventory.empty')}</div>
          ) : (
            <div className="probe-list">
              {filteredModelInventory.map((item) => (
                <div key={`${item.provider}-${item.id}`} className="probe-check">
                  <span>{item.id}</span>
                  <strong>{item.provider} · {presentProbeStatus(item.probe, t)}</strong>
                </div>
              ))}
            </div>
          )}
        </Panel>
      </div>
    </>
  );
}
