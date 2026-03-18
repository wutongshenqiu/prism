import { WorkbenchSheet } from '../WorkbenchSheet';
import { useI18n } from '../../i18n';
import { presentProbeStatus, presentProviderFormat } from '../../lib/operatorPresentation';
import type { ProviderAtlasRow } from '../../types/controlPlane';
import type { ProviderRegistryFormState } from './types';

interface ProviderRegistrySheetProps {
  open: boolean;
  registryStatus: string | null;
  registryError: string | null;
  registryLoading: boolean;
  registryForm: ProviderRegistryFormState;
  selectedProvider: string | null;
  selectedRow: ProviderAtlasRow | null;
  selectedProbeStatus: string | null;
  onClose: () => void;
  onRegistryFormChange: (patch: Partial<ProviderRegistryFormState>) => void;
  onFetchModels: () => void;
  onDeleteSelectedProvider: () => void;
  onCreateProvider: () => void;
}

export function ProviderRegistrySheet({
  open,
  registryStatus,
  registryError,
  registryLoading,
  registryForm,
  selectedProvider,
  selectedRow,
  selectedProbeStatus,
  onClose,
  onRegistryFormChange,
  onFetchModels,
  onDeleteSelectedProvider,
  onCreateProvider,
}: ProviderRegistrySheetProps) {
  const { t, tx } = useI18n();

  return (
    <WorkbenchSheet
      open={open}
      onClose={onClose}
      title={t('providerAtlas.registry.title')}
      subtitle={t('providerAtlas.registry.subtitle')}
      actions={(
        <>
          <button type="button" className="button button--ghost" onClick={onFetchModels} disabled={registryLoading}>
            {registryLoading ? t('providerAtlas.registry.working') : t('providerAtlas.registry.fetchModels')}
          </button>
          <button type="button" className="button button--ghost" onClick={onDeleteSelectedProvider} disabled={registryLoading || !selectedProvider}>
            {t('providerAtlas.registry.deleteSelected')}
          </button>
          <button type="button" className="button button--primary" onClick={onCreateProvider} disabled={registryLoading}>
            {registryLoading ? t('providerAtlas.registry.saving') : t('providerAtlas.registry.createProvider')}
          </button>
        </>
      )}
    >
      {registryStatus ? <div className="status-message status-message--success">{registryStatus}</div> : null}
      {registryError ? <div className="status-message status-message--danger">{registryError}</div> : null}

      <section className="sheet-section">
        <h3>{t('providerAtlas.registry.newProviderDraft')}</h3>
        <form
          className="sheet-form"
          onSubmit={(event) => {
            event.preventDefault();
            onCreateProvider();
          }}
        >
          <label className="sheet-field">
            <span>{t('common.name')}</span>
            <input
              name="provider-name"
              autoComplete="organization"
              value={registryForm.name}
              onChange={(event) => onRegistryFormChange({ name: event.target.value })}
            />
          </label>
          <label className="sheet-field">
            <span>{t('common.format')}</span>
            <select value={registryForm.format} onChange={(event) => onRegistryFormChange({ format: event.target.value as ProviderRegistryFormState['format'] })}>
              <option value="openai">{presentProviderFormat('openai')}</option>
              <option value="claude">{presentProviderFormat('claude')}</option>
              <option value="gemini">{presentProviderFormat('gemini')}</option>
            </select>
          </label>
          <label className="sheet-field">
            <span>{t('common.upstream')}</span>
            <input
              name="provider-upstream"
              autoComplete="off"
              value={registryForm.upstream}
              onChange={(event) => onRegistryFormChange({ upstream: event.target.value })}
            />
          </label>
          <label className="sheet-field">
            <span>{t('providerAtlas.registry.apiKey')}</span>
            <input
              name="provider-api-key"
              type="password"
              autoComplete="new-password"
              value={registryForm.apiKey}
              onChange={(event) => onRegistryFormChange({ apiKey: event.target.value })}
            />
          </label>
          <label className="sheet-field">
            <span>{t('providerAtlas.editor.baseUrl')}</span>
            <input
              name="registry-base-url"
              type="url"
              autoComplete="url"
              value={registryForm.baseUrl}
              onChange={(event) => onRegistryFormChange({ baseUrl: event.target.value })}
            />
          </label>
          <label className="sheet-field">
            <span>{t('common.models')}</span>
            <input
              name="provider-models"
              autoComplete="off"
              value={registryForm.models}
              onChange={(event) => onRegistryFormChange({ models: event.target.value })}
            />
          </label>
          <label className="detail-grid__row">
            <span>{t('common.disabled')}</span>
            <input
              type="checkbox"
              checked={registryForm.disabled}
              onChange={(event) => onRegistryFormChange({ disabled: event.target.checked })}
            />
          </label>
        </form>
      </section>

      <section className="sheet-section">
        <h3>{t('providerAtlas.registry.selectedProvider')}</h3>
          <div className="detail-grid">
            <div className="detail-grid__row"><span>{t('common.name')}</span><strong>{selectedProvider ?? t('common.noneSelected')}</strong></div>
            <div className="detail-grid__row"><span>{t('common.status')}</span><strong>{selectedRow ? tx(selectedRow.status) : t('common.notAvailable')}</strong></div>
            <div className="detail-grid__row"><span>{t('providerAtlas.table.auth')}</span><strong>{selectedRow ? tx(selectedRow.auth) : t('common.notAvailable')}</strong></div>
            <div className="detail-grid__row"><span>{t('providerAtlas.registry.coverage')}</span><strong>{selectedProbeStatus ? presentProbeStatus(selectedProbeStatus, t) : t('common.notAvailable')}</strong></div>
          </div>
        </section>
    </WorkbenchSheet>
  );
}
