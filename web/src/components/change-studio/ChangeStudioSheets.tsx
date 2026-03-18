import { WorkbenchSheet } from '../WorkbenchSheet';
import { useI18n } from '../../i18n';
import type { AccessPolicyFormState } from '../../lib/authKeyPolicy';
import type { RouteDraft } from '../../lib/routeDraft';
import type {
  AuthKeySummary,
  ConfigApplyResponse,
  ConfigValidateResponse,
} from '../../types/backend';
import type { RegistryRow } from '../../types/controlPlane';

interface ChangeEditorSheetProps {
  open: boolean;
  editorMode: 'structured' | 'yaml';
  loadingEditor: boolean;
  actionStatus: string | null;
  actionError: string | null;
  validating: boolean;
  reloading: boolean;
  applying: boolean;
  yaml: string;
  configPath: string;
  configVersion?: string;
  selectedRegistry: RegistryRow | null;
  routeDraft: RouteDraft | null;
  validationResult: ConfigValidateResponse | null;
  applyResult: ConfigApplyResponse | null;
  onClose: () => void;
  onValidate: () => void;
  onReloadRuntime: () => void;
  onApply: () => void;
  onYamlChange: (value: string) => void;
  onDiscardRouteDraft: () => void;
}

export function ChangeEditorSheet({
  open,
  editorMode,
  loadingEditor,
  actionStatus,
  actionError,
  validating,
  reloading,
  applying,
  yaml,
  configPath,
  configVersion,
  selectedRegistry,
  routeDraft,
  validationResult,
  applyResult,
  onClose,
  onValidate,
  onReloadRuntime,
  onApply,
  onYamlChange,
  onDiscardRouteDraft,
}: ChangeEditorSheetProps) {
  const { t, tx, formatDateTime } = useI18n();

  return (
    <WorkbenchSheet
      open={open}
      onClose={onClose}
      title={editorMode === 'structured' ? t('changeStudio.editor.structuredTitle') : t('changeStudio.editor.yamlTitle')}
      subtitle={t('changeStudio.editor.subtitle')}
      actions={(
        <>
          <button type="button" className="button button--ghost" onClick={onValidate} disabled={validating || loadingEditor}>
            {validating ? t('changeStudio.editor.validating') : t('changeStudio.editor.validate')}
          </button>
          <button type="button" className="button button--ghost" onClick={onReloadRuntime} disabled={reloading || loadingEditor}>
            {reloading ? t('changeStudio.editor.reloading') : t('changeStudio.editor.reloadRuntime')}
          </button>
          <button type="button" className="button button--primary" onClick={onApply} disabled={applying || loadingEditor}>
            {applying ? t('changeStudio.editor.applying') : t('changeStudio.editor.applyDraft')}
          </button>
        </>
      )}
    >
      {loadingEditor ? <div className="status-message">{t('changeStudio.editor.loading')}</div> : null}
      {actionStatus ? <div className="status-message status-message--success">{actionStatus}</div> : null}
      {actionError ? <div className="status-message status-message--danger">{actionError}</div> : null}

      <section className="sheet-section">
        <h3>{t('changeStudio.editor.brief')}</h3>
        <div className="detail-grid">
          <div className="detail-grid__row"><span>{t('common.mode')}</span><strong>{editorMode === 'structured' ? t('changeStudio.editor.modeStructured') : t('changeStudio.editor.modeYaml')}</strong></div>
          <div className="detail-grid__row"><span>{t('common.family')}</span><strong>{selectedRegistry ? tx(selectedRegistry.family_label) : t('common.noneSelected')}</strong></div>
          <div className="detail-grid__row"><span>{t('common.record')}</span><strong>{selectedRegistry?.record ?? t('common.notAvailable')}</strong></div>
          <div className="detail-grid__row"><span>{t('changeStudio.editor.configPath')}</span><strong>{configPath || t('common.loading')}</strong></div>
          <div className="detail-grid__row"><span>{t('common.version')}</span><strong>{configVersion ?? t('changeStudio.editor.versionPending')}</strong></div>
        </div>
        {editorMode === 'structured' ? (
          <div className="status-message">{t('changeStudio.editor.structuredNote')}</div>
        ) : null}
      </section>

      {routeDraft ? (
        <section className="sheet-section">
          <h3>{t('changeStudio.editor.linkedRouteDraft')}</h3>
          <div className="detail-grid">
            <div className="detail-grid__row"><span>{t('routeStudio.scenario.scenario')}</span><strong>{routeDraft.scenario.scenario}</strong></div>
            <div className="detail-grid__row"><span>{t('routeStudio.scenario.winner')}</span><strong>{routeDraft.explanation?.selected?.provider ?? routeDraft.scenario.winner}</strong></div>
            <div className="detail-grid__row"><span>{t('changeStudio.editor.createdAt')}</span><strong>{formatDateTime(routeDraft.createdAt)}</strong></div>
          </div>
          <div className="inline-actions">
            <button type="button" className="button button--ghost" onClick={onDiscardRouteDraft}>
              {t('changeStudio.editor.discardLinkedDraft')}
            </button>
          </div>
        </section>
      ) : null}

      <section className="sheet-section">
        <h3>{t('changeStudio.editor.configTransaction')}</h3>
        <textarea className="yaml-editor" value={yaml} onChange={(event) => onYamlChange(event.target.value)} spellCheck={false} />
      </section>

      {validationResult ? (
        <section className="sheet-section">
          <h3>{t('changeStudio.editor.validationResult')}</h3>
          <div className={`status-message ${validationResult.valid ? 'status-message--success' : 'status-message--warning'}`}>
            {validationResult.valid ? t('changeStudio.editor.validationValid') : t('changeStudio.editor.validationIssues')}
          </div>
          {validationResult.errors.length > 0 ? (
            <div className="yaml-errors">
              {validationResult.errors.map((item) => (
                <div key={item} className="probe-check">
                  <span>{t('changeStudio.editor.issue')}</span>
                  <strong>{item}</strong>
                </div>
              ))}
            </div>
          ) : null}
        </section>
      ) : null}

      {applyResult ? (
        <section className="sheet-section">
          <h3>{t('changeStudio.editor.lastApply')}</h3>
          <div className="detail-grid">
            <div className="detail-grid__row"><span>{t('changeStudio.editor.message')}</span><strong>{applyResult.message}</strong></div>
            <div className="detail-grid__row"><span>{t('changeStudio.editor.configVersion')}</span><strong>{applyResult.config_version}</strong></div>
          </div>
        </section>
      ) : null}
    </WorkbenchSheet>
  );
}

interface AccessControlSheetProps {
  open: boolean;
  accessEditorMode: 'create' | 'edit';
  accessStatus: string | null;
  accessError: string | null;
  revealedKey: string | null;
  revealedCountdown: number | null;
  revealingKey: boolean;
  deletingKey: boolean;
  savingKey: boolean;
  accessForm: AccessPolicyFormState;
  selectedAuthKey: AuthKeySummary | null;
  authKeys: AuthKeySummary[];
  selectedAuthKeyId: number | null;
  onClose: () => void;
  onStartNewDraft: () => void;
  onRevealSelected: () => void;
  onDeleteSelected: () => void;
  onSaveKey: () => void;
  onAccessFormChange: (patch: Partial<AccessPolicyFormState>) => void;
  onSelectAuthKey: (authKeyId: number) => void;
}

export function AccessControlSheet({
  open,
  accessEditorMode,
  accessStatus,
  accessError,
  revealedKey,
  revealedCountdown,
  revealingKey,
  deletingKey,
  savingKey,
  accessForm,
  selectedAuthKey,
  authKeys,
  selectedAuthKeyId,
  onClose,
  onStartNewDraft,
  onRevealSelected,
  onDeleteSelected,
  onSaveKey,
  onAccessFormChange,
  onSelectAuthKey,
}: AccessControlSheetProps) {
  const { t } = useI18n();

  return (
    <WorkbenchSheet
      open={open}
      onClose={onClose}
      title={t('changeStudio.access.title')}
      subtitle={t('changeStudio.access.subtitle')}
      actions={(
        <>
          <button type="button" className="button button--ghost" onClick={onStartNewDraft}>
            {t('changeStudio.access.newDraft')}
          </button>
          <button type="button" className="button button--primary" onClick={onSaveKey} disabled={savingKey}>
            {savingKey ? t('changeStudio.access.saving') : accessEditorMode === 'edit' ? t('changeStudio.access.saveKey') : t('changeStudio.access.createKey')}
          </button>
        </>
      )}
    >
      {accessStatus ? <div className="status-message status-message--success">{accessStatus}</div> : null}
      {accessError ? <div className="status-message status-message--danger">{accessError}</div> : null}
      {revealedKey ? (
        <div className="status-message status-message--warning">
          {t('changeStudio.access.revealedNow')} <strong>{revealedKey}</strong>
          {revealedCountdown !== null ? <span style={{ marginLeft: 8, opacity: 0.75 }}>{t('changeStudio.access.revealedCountdown', { seconds: revealedCountdown })}</span> : null}
        </div>
      ) : null}

      <section className="sheet-section">
        <h3>{accessEditorMode === 'edit' ? t('changeStudio.access.editPolicy') : t('changeStudio.access.createKey')}</h3>
        <div className="sheet-form">
          <label className="sheet-field"><span>{t('common.name')}</span><input name="auth-key-name" autoComplete="off" value={accessForm.name} onChange={(event) => onAccessFormChange({ name: event.target.value })} /></label>
          <label className="sheet-field"><span>{t('changeStudio.access.tenantId')}</span><input name="auth-key-tenant-id" autoComplete="off" value={accessForm.tenantId} onChange={(event) => onAccessFormChange({ tenantId: event.target.value })} /></label>
          <label className="sheet-field"><span>{t('changeStudio.access.allowedModels')}</span><input name="auth-key-models" autoComplete="off" value={accessForm.allowedModels} onChange={(event) => onAccessFormChange({ allowedModels: event.target.value })} /></label>
          <label className="sheet-field"><span>{t('changeStudio.access.allowedCredentials')}</span><input name="auth-key-credentials" autoComplete="off" value={accessForm.allowedCredentials} onChange={(event) => onAccessFormChange({ allowedCredentials: event.target.value })} /></label>
          <label className="sheet-field"><span>{t('changeStudio.access.rpm')}</span><input name="auth-key-rpm" inputMode="numeric" autoComplete="off" value={accessForm.rpm} onChange={(event) => onAccessFormChange({ rpm: event.target.value })} /></label>
          <label className="sheet-field"><span>{t('changeStudio.access.tpm')}</span><input name="auth-key-tpm" inputMode="numeric" autoComplete="off" value={accessForm.tpm} onChange={(event) => onAccessFormChange({ tpm: event.target.value })} /></label>
          <label className="sheet-field"><span>{t('changeStudio.access.costPerDay')}</span><input name="auth-key-cost-per-day" inputMode="decimal" autoComplete="off" value={accessForm.costPerDayUsd} onChange={(event) => onAccessFormChange({ costPerDayUsd: event.target.value })} /></label>
          <label className="sheet-field"><span>{t('changeStudio.access.expiresAt')}</span><input name="auth-key-expires-at" type="datetime-local" value={accessForm.expiresAt} onChange={(event) => onAccessFormChange({ expiresAt: event.target.value })} /></label>
          <label className="detail-grid__row"><span>{t('changeStudio.access.budgetEnabled')}</span><input type="checkbox" checked={accessForm.budgetEnabled} onChange={(event) => onAccessFormChange({ budgetEnabled: event.target.checked })} /></label>
          <label className="sheet-field"><span>{t('changeStudio.access.budgetTotal')}</span><input name="auth-key-budget-total" inputMode="decimal" autoComplete="off" value={accessForm.budgetTotalUsd} onChange={(event) => onAccessFormChange({ budgetTotalUsd: event.target.value })} /></label>
          <label className="sheet-field">
            <span>{t('changeStudio.access.budgetPeriod')}</span>
            <select value={accessForm.budgetPeriod} onChange={(event) => onAccessFormChange({ budgetPeriod: event.target.value as AccessPolicyFormState['budgetPeriod'] })}>
              <option value="daily">{t('changeStudio.access.period.daily')}</option>
              <option value="monthly">{t('changeStudio.access.period.monthly')}</option>
            </select>
          </label>
        </div>
      </section>

      {selectedAuthKey ? (
        <section className="sheet-section">
          <h3>{t('changeStudio.access.selectedKeyPosture')}</h3>
          <div className="detail-grid">
            <div className="detail-grid__row"><span>{t('changeStudio.access.key')}</span><strong>{selectedAuthKey.key_masked}</strong></div>
            <div className="detail-grid__row"><span>{t('common.tenant')}</span><strong>{selectedAuthKey.tenant_id ?? t('common.global')}</strong></div>
            <div className="detail-grid__row"><span>{t('changeStudio.access.modelAllowlist')}</span><strong>{selectedAuthKey.allowed_models.length || t('common.all')}</strong></div>
            <div className="detail-grid__row"><span>{t('changeStudio.access.credentialAllowlist')}</span><strong>{selectedAuthKey.allowed_credentials.length || t('common.all')}</strong></div>
          </div>
          <div className="sheet-form">
            <h4>{t('changeStudio.access.selectedKeyActions')}</h4>
            <div className="inline-actions inline-actions--wrap">
              <button type="button" className="button button--ghost" onClick={onRevealSelected} disabled={revealingKey}>
                {revealingKey ? t('changeStudio.access.revealing') : t('changeStudio.access.revealSelected')}
              </button>
              <button type="button" className="button button--ghost" onClick={onDeleteSelected} disabled={deletingKey}>
                {deletingKey ? t('changeStudio.access.deleting') : t('changeStudio.access.deleteSelected')}
              </button>
            </div>
          </div>
        </section>
      ) : null}

      <section className="sheet-section">
        <h3>{t('changeStudio.access.existingKeys')}</h3>
        <div className="probe-list">
          {authKeys.length === 0 ? (
            <div className="probe-check">
              <span>{t('changeStudio.access.keyPlural')}</span>
              <strong>{t('changeStudio.access.noneConfigured')}</strong>
            </div>
          ) : (
            authKeys.map((item) => (
              <div key={item.id} className={`probe-check ${item.id === selectedAuthKeyId ? 'probe-check--selected' : ''}`}>
                <span>{item.key_masked}</span>
                <strong>{item.name ?? t('changeStudio.access.unnamed')} · {item.tenant_id ?? t('common.global')}</strong>
                <button type="button" className="button button--ghost" onClick={() => onSelectAuthKey(item.id)}>
                  {t('common.select')}
                </button>
              </div>
            ))
          )}
        </div>
      </section>
    </WorkbenchSheet>
  );
}
