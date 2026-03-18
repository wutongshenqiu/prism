import { WorkbenchSheet } from '../WorkbenchSheet';
import { profileKey } from '../../lib/authProfileDraft';
import { PayloadViewer } from '../PayloadViewer';
import { useI18n } from '../../i18n';
import {
  presentAuthMode,
  presentCapabilityName,
  presentMutationKind,
  presentPresentationMode,
  presentPresentationProfile,
  presentProbeStatus,
  presentProviderFormat,
  presentWireApi,
} from '../../lib/operatorPresentation';
import type {
  AuthProfilesRuntimeResponse,
  PresentationPreviewResponse,
  ProviderCapabilityEntry,
  ProviderDetail,
  ProviderHealthResult,
  ProviderTestResponse,
} from '../../types/backend';
import type { ProviderEditorFormState, ProviderTestFormState } from './types';

interface ProviderEditorSheetProps {
  open: boolean;
  loadingDetail: boolean;
  actionStatus: string | null;
  actionError: string | null;
  detail: ProviderDetail | null;
  runtimeInfo: AuthProfilesRuntimeResponse | null;
  health: ProviderHealthResult | null;
  preview: PresentationPreviewResponse | null;
  previewing: boolean;
  saving: boolean;
  testingRequest: boolean;
  selectedCapabilities: ProviderCapabilityEntry | null;
  formState: ProviderEditorFormState;
  testForm: ProviderTestFormState;
  testResult: ProviderTestResponse | null;
  testError: string | null;
  refreshingProfileId: string | null;
  onClose: () => void;
  onRunHealthCheck: () => void;
  onRunPresentationPreview: () => void;
  onRunTestRequest: () => void;
  onSaveProvider: () => void;
  onFormStateChange: (patch: Partial<ProviderEditorFormState>) => void;
  onTestFormChange: (patch: Partial<ProviderTestFormState>) => void;
  onRefreshAuthProfile: (provider: string, profileId: string) => void;
}

export function ProviderEditorSheet({
  open,
  loadingDetail,
  actionStatus,
  actionError,
  detail,
  runtimeInfo,
  health,
  preview,
  previewing,
  saving,
  testingRequest,
  selectedCapabilities,
  formState,
  testForm,
  testResult,
  testError,
  refreshingProfileId,
  onClose,
  onRunHealthCheck,
  onRunPresentationPreview,
  onRunTestRequest,
  onSaveProvider,
  onFormStateChange,
  onTestFormChange,
  onRefreshAuthProfile,
}: ProviderEditorSheetProps) {
  const { t, formatDateTime, formatDurationMs, formatNumber } = useI18n();

  return (
    <WorkbenchSheet
      open={open}
      onClose={onClose}
      title={t('providerAtlas.editor.title')}
      subtitle={t('providerAtlas.editor.subtitle')}
      actions={(
        <button type="button" className="button button--primary" onClick={onSaveProvider} disabled={saving}>
          {saving ? t('providerAtlas.editor.saving') : t('providerAtlas.editor.saveProvider')}
        </button>
      )}
    >
      {loadingDetail ? <div className="status-message">{t('providerAtlas.editor.loadingDetail')}</div> : null}
      {actionStatus ? <div className="status-message status-message--success">{actionStatus}</div> : null}
      {actionError ? <div className="status-message status-message--danger">{actionError}</div> : null}

      {detail ? (
        <>
          <section className="sheet-section">
            <h3>{t('providerAtlas.editor.providerPosture')}</h3>
            <div className="detail-grid">
              <div className="detail-grid__row"><span>{t('common.name')}</span><strong>{detail.name}</strong></div>
              <div className="detail-grid__row"><span>{t('common.format')}</span><strong>{presentProviderFormat(detail.format)}</strong></div>
              <div className="detail-grid__row"><span>{t('common.upstream')}</span><strong>{detail.upstream}</strong></div>
              <div className="detail-grid__row"><span>{t('providerAtlas.editor.authProfiles')}</span><strong>{formatNumber(detail.auth_profiles.length)}</strong></div>
            </div>
          </section>

          <section className="sheet-section">
            <h3>{t('providerAtlas.editor.editableRuntimeFields')}</h3>
            <div className="sheet-form">
              <label className="sheet-field">
                <span>{t('providerAtlas.editor.baseUrl')}</span>
                <input
                  name="provider-base-url"
                  type="url"
                  autoComplete="url"
                  value={formState.baseUrl}
                  onChange={(event) => onFormStateChange({ baseUrl: event.target.value })}
                />
              </label>
              <label className="sheet-field">
                <span>{t('common.region')}</span>
                <input
                  name="provider-region"
                  autoComplete="off"
                  value={formState.region}
                  onChange={(event) => onFormStateChange({ region: event.target.value })}
                />
              </label>
              <label className="sheet-field">
                <span>{t('common.weight')}</span>
                <input
                  name="provider-weight"
                  inputMode="numeric"
                  autoComplete="off"
                  value={formState.weight}
                  onChange={(event) => onFormStateChange({ weight: event.target.value })}
                />
              </label>
              <label className="detail-grid__row">
                <span>{t('common.disabled')}</span>
                <input
                  type="checkbox"
                  checked={formState.disabled}
                  onChange={(event) => onFormStateChange({ disabled: event.target.checked })}
                />
              </label>
            </div>
          </section>

          <section className="sheet-section">
            <h3>{t('providerAtlas.editor.liveOps')}</h3>
            <div className="inline-actions inline-actions--wrap">
              <button type="button" className="button button--ghost" onClick={onRunHealthCheck}>
                {t('providerAtlas.editor.runHealthProbe')}
              </button>
              <button type="button" className="button button--ghost" onClick={onRunPresentationPreview} disabled={previewing}>
                {previewing ? t('providerAtlas.editor.previewing') : t('providerAtlas.editor.presentationPreview')}
              </button>
              <button type="button" className="button button--secondary" onClick={onRunTestRequest} disabled={testingRequest}>
                {testingRequest ? t('providerAtlas.editor.testingRequest') : t('providerAtlas.editor.sendTestRequest')}
              </button>
            </div>
            <div className="sheet-form">
              <label className="sheet-field">
                <span>{t('common.model')}</span>
                <input
                  name="provider-test-model"
                  autoComplete="off"
                  value={testForm.model}
                  onChange={(event) => onTestFormChange({ model: event.target.value })}
                />
              </label>
              <label className="sheet-field">
                <span>{t('providerAtlas.editor.testInput')}</span>
                <textarea
                  name="provider-test-input"
                  rows={4}
                  value={testForm.input}
                  onChange={(event) => onTestFormChange({ input: event.target.value })}
                />
              </label>
            </div>
            {testError ? <div className="status-message status-message--danger">{testError}</div> : null}
            {testResult ? (
              <>
                <div className="detail-grid">
                  <div className="detail-grid__row"><span>{t('common.path')}</span><strong>{testResult.endpoint}</strong></div>
                  <div className="detail-grid__row"><span>{t('common.status')}</span><strong>{testResult.status}</strong></div>
                  <div className="detail-grid__row"><span>{t('common.latency')}</span><strong>{formatDurationMs(testResult.latency_ms)}</strong></div>
                </div>
                <PayloadViewer
                  title={t('providerAtlas.editor.testRequestPayload')}
                  payload={testResult.request_body}
                  emptyMessage={t('providerAtlas.editor.noTestRequestPayload')}
                />
                <PayloadViewer
                  title={t('providerAtlas.editor.testResponsePayload')}
                  payload={testResult.response_body}
                  emptyMessage={t('providerAtlas.editor.noTestResponsePayload')}
                />
              </>
            ) : null}
          </section>

          <section className="sheet-section">
            <h3>{t('providerAtlas.editor.authProfiles')}</h3>
            <div className="probe-list">
              {detail.auth_profiles.length === 0 ? (
                <div className="probe-check">
                  <span>{t('providerAtlas.editor.authProfiles')}</span>
                  <strong>{t('providerAtlas.editor.noneConfigured')}</strong>
                </div>
              ) : (
                detail.auth_profiles.map((profile) => (
                  <div key={profile.qualified_name} className="probe-check">
                    <span>{profile.qualified_name}</span>
                    <strong>{presentAuthMode(profile.mode, t)}</strong>
                    {profile.refresh_token_present ? (
                      <button
                        type="button"
                        className="button button--ghost"
                        onClick={() => onRefreshAuthProfile(detail.name, profile.id)}
                        disabled={refreshingProfileId === profileKey(detail.name, profile.id)}
                      >
                        {refreshingProfileId === profileKey(detail.name, profile.id)
                          ? t('providerAtlas.editor.refreshing')
                          : t('common.refresh')}
                      </button>
                    ) : null}
                  </div>
                ))
              )}
            </div>
          </section>

          {runtimeInfo ? (
            <section className="sheet-section">
              <h3>{t('providerAtlas.editor.managedAuthRuntime')}</h3>
              <div className="detail-grid">
                <div className="detail-grid__row"><span>{t('providerAtlas.editor.storageDir')}</span><strong>{runtimeInfo.storage_dir ?? t('common.notConfigured')}</strong></div>
                <div className="detail-grid__row"><span>{t('providerAtlas.editor.codexAuthFile')}</span><strong>{runtimeInfo.codex_auth_file ?? t('common.notConfigured')}</strong></div>
                <div className="detail-grid__row"><span>{t('providerAtlas.editor.proxyUrl')}</span><strong>{runtimeInfo.proxy_url ?? t('common.none')}</strong></div>
              </div>
            </section>
          ) : null}

          {selectedCapabilities ? (
            <section className="sheet-section">
              <h3>{t('providerAtlas.editor.capabilitySnapshot')}</h3>
              <div className="detail-grid">
                <div className="detail-grid__row"><span>{t('providerAtlas.coverage.probeStatus')}</span><strong>{presentProbeStatus(selectedCapabilities.probe_status, t)}</strong></div>
                <div className="detail-grid__row"><span>{t('providerAtlas.coverage.presentation')}</span><strong>{`${presentPresentationProfile(selectedCapabilities.presentation_profile, t)} / ${presentPresentationMode(selectedCapabilities.presentation_mode, t)}`}</strong></div>
                <div className="detail-grid__row"><span>{t('common.models')}</span><strong>{formatNumber(selectedCapabilities.models.length)}</strong></div>
                <div className="detail-grid__row"><span>{t('providerAtlas.editor.wireApi')}</span><strong>{presentWireApi(selectedCapabilities.wire_api, t)}</strong></div>
              </div>
            </section>
          ) : null}

          {health ? (
            <section className="sheet-section">
              <h3>{t('providerAtlas.editor.healthProbe')}</h3>
              <div className="detail-grid">
                <div className="detail-grid__row"><span>{t('common.status')}</span><strong>{presentProbeStatus(health.status, t)}</strong></div>
                <div className="detail-grid__row"><span>{t('providerAtlas.editor.checkedAt')}</span><strong>{formatDateTime(health.checked_at)}</strong></div>
                <div className="detail-grid__row"><span>{t('common.latency')}</span><strong>{formatDurationMs(health.latency_ms)}</strong></div>
              </div>
              <div className="probe-list">
                {health.checks.map((check) => (
                  <div key={check.capability} className="probe-check">
                    <span>{presentCapabilityName(check.capability, t)}</span>
                    <strong>{presentProbeStatus(check.status, t)}</strong>
                  </div>
                ))}
              </div>
            </section>
          ) : null}

          {preview ? (
            <section className="sheet-section">
              <h3>{t('providerAtlas.editor.presentationPreview')}</h3>
              <div className="detail-grid">
                <div className="detail-grid__row"><span>{t('common.profile')}</span><strong>{preview.profile}</strong></div>
                <div className="detail-grid__row"><span>{t('providerAtlas.editor.activated')}</span><strong>{preview.activated ? t('common.yes') : t('common.no')}</strong></div>
                <div className="detail-grid__row"><span>{t('providerAtlas.editor.protectedHeadersBlocked')}</span><strong>{formatNumber(preview.protected_headers_blocked.length)}</strong></div>
                <div className="detail-grid__row"><span>{t('providerAtlas.editor.mutations')}</span><strong>{formatNumber(preview.body_mutations.length)}</strong></div>
              </div>
              <div className="probe-list">
                {preview.body_mutations.map((mutation) => (
                  <div key={`${mutation.kind}-${mutation.reason ?? 'none'}`} className="probe-check">
                    <span>{presentMutationKind(mutation.kind, t)}</span>
                    <strong>{mutation.applied ? t('providerAtlas.editor.applied') : mutation.reason ?? t('providerAtlas.editor.skipped')}</strong>
                  </div>
                ))}
              </div>
              <PayloadViewer
                title={t('providerAtlas.editor.effectiveBody')}
                payload={preview.effective_body}
                emptyMessage={t('providerAtlas.editor.noEffectiveBody')}
              />
            </section>
          ) : null}
        </>
      ) : null}
    </WorkbenchSheet>
  );
}
