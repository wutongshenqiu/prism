import { WorkbenchSheet } from '../WorkbenchSheet';
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
  return (
    <WorkbenchSheet
      open={open}
      onClose={onClose}
      title={editorMode === 'structured' ? 'Structured change workbench' : 'YAML transaction workbench'}
      subtitle="Every change follows the same loop: load current truth, validate it, apply it, then observe runtime reload."
      actions={(
        <>
          <button type="button" className="button button--ghost" onClick={onValidate} disabled={validating || loadingEditor}>
            {validating ? 'Validating…' : 'Validate'}
          </button>
          <button type="button" className="button button--ghost" onClick={onReloadRuntime} disabled={reloading || loadingEditor}>
            {reloading ? 'Reloading…' : 'Reload runtime'}
          </button>
          <button type="button" className="button button--primary" onClick={onApply} disabled={applying || loadingEditor}>
            {applying ? 'Applying…' : 'Apply draft'}
          </button>
        </>
      )}
    >
      {loadingEditor ? <div className="status-message">Loading current config and linked drafts…</div> : null}
      {actionStatus ? <div className="status-message status-message--success">{actionStatus}</div> : null}
      {actionError ? <div className="status-message status-message--danger">{actionError}</div> : null}

      <section className="sheet-section">
        <h3>Change brief</h3>
        <div className="detail-grid">
          <div className="detail-grid__row"><span>Mode</span><strong>{editorMode === 'structured' ? 'structured' : 'yaml'}</strong></div>
          <div className="detail-grid__row"><span>Family</span><strong>{selectedRegistry?.family ?? 'none selected'}</strong></div>
          <div className="detail-grid__row"><span>Record</span><strong>{selectedRegistry?.record ?? 'n/a'}</strong></div>
          <div className="detail-grid__row"><span>Config path</span><strong>{configPath || 'loading…'}</strong></div>
          <div className="detail-grid__row"><span>Version</span><strong>{configVersion ?? 'pending'}</strong></div>
        </div>
        {editorMode === 'structured' ? (
          <div className="status-message">
            Structured mode keeps operator intent, affected family, and linked route context visible while the transaction is still applied as first-class YAML.
          </div>
        ) : null}
      </section>

      {routeDraft ? (
        <section className="sheet-section">
          <h3>Linked route draft</h3>
          <div className="detail-grid">
            <div className="detail-grid__row"><span>Scenario</span><strong>{routeDraft.scenario.scenario}</strong></div>
            <div className="detail-grid__row"><span>Winner</span><strong>{routeDraft.explanation?.selected?.provider ?? routeDraft.scenario.winner}</strong></div>
            <div className="detail-grid__row"><span>Created at</span><strong>{routeDraft.createdAt}</strong></div>
          </div>
          <div className="inline-actions">
            <button type="button" className="button button--ghost" onClick={onDiscardRouteDraft}>
              Discard linked draft
            </button>
          </div>
        </section>
      ) : null}

      <section className="sheet-section">
        <h3>Config transaction</h3>
        <textarea
          className="yaml-editor"
          value={yaml}
          onChange={(event) => onYamlChange(event.target.value)}
          spellCheck={false}
        />
      </section>

      {validationResult ? (
        <section className="sheet-section">
          <h3>Validation result</h3>
          <div className={`status-message ${validationResult.valid ? 'status-message--success' : 'status-message--warning'}`}>
            {validationResult.valid ? 'Configuration is valid.' : 'Validation returned issues.'}
          </div>
          {validationResult.errors.length > 0 ? (
            <div className="yaml-errors">
              {validationResult.errors.map((item) => (
                <div key={item} className="probe-check">
                  <span>Issue</span>
                  <strong>{item}</strong>
                </div>
              ))}
            </div>
          ) : null}
        </section>
      ) : null}

      {applyResult ? (
        <section className="sheet-section">
          <h3>Last apply</h3>
          <div className="detail-grid">
            <div className="detail-grid__row"><span>Message</span><strong>{applyResult.message}</strong></div>
            <div className="detail-grid__row"><span>Config version</span><strong>{applyResult.config_version}</strong></div>
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
  return (
    <WorkbenchSheet
      open={open}
      onClose={onClose}
      title="Access control workbench"
      subtitle="Create, edit, reveal, and revoke gateway auth keys while keeping tenant scope and budgets visible."
      actions={(
        <>
          <button type="button" className="button button--ghost" onClick={onStartNewDraft}>
            New draft
          </button>
          <button type="button" className="button button--ghost" onClick={onRevealSelected} disabled={revealingKey}>
            {revealingKey ? 'Revealing…' : 'Reveal selected'}
          </button>
          <button type="button" className="button button--ghost" onClick={onDeleteSelected} disabled={deletingKey}>
            {deletingKey ? 'Deleting…' : 'Delete selected'}
          </button>
          <button type="button" className="button button--primary" onClick={onSaveKey} disabled={savingKey}>
            {savingKey ? 'Saving…' : accessEditorMode === 'edit' ? 'Save key' : 'Create key'}
          </button>
        </>
      )}
    >
      {accessStatus ? <div className="status-message status-message--success">{accessStatus}</div> : null}
      {accessError ? <div className="status-message status-message--danger">{accessError}</div> : null}
      {revealedKey ? <div className="status-message status-message--warning">Save this key now: <strong>{revealedKey}</strong></div> : null}

      <section className="sheet-section">
        <h3>{accessEditorMode === 'edit' ? 'Edit key policy' : 'Create key'}</h3>
        <div className="sheet-form">
          <label className="sheet-field">
            <span>Name</span>
            <input
              name="auth-key-name"
              autoComplete="off"
              value={accessForm.name}
              onChange={(event) => onAccessFormChange({ name: event.target.value })}
            />
          </label>
          <label className="sheet-field">
            <span>Tenant ID</span>
            <input
              name="auth-key-tenant-id"
              autoComplete="off"
              value={accessForm.tenantId}
              onChange={(event) => onAccessFormChange({ tenantId: event.target.value })}
            />
          </label>
          <label className="sheet-field">
            <span>Allowed models</span>
            <input
              name="auth-key-models"
              autoComplete="off"
              value={accessForm.allowedModels}
              onChange={(event) => onAccessFormChange({ allowedModels: event.target.value })}
            />
          </label>
          <label className="sheet-field">
            <span>Allowed credentials</span>
            <input
              name="auth-key-credentials"
              autoComplete="off"
              value={accessForm.allowedCredentials}
              onChange={(event) => onAccessFormChange({ allowedCredentials: event.target.value })}
            />
          </label>
          <label className="sheet-field">
            <span>RPM</span>
            <input
              name="auth-key-rpm"
              inputMode="numeric"
              autoComplete="off"
              value={accessForm.rpm}
              onChange={(event) => onAccessFormChange({ rpm: event.target.value })}
            />
          </label>
          <label className="sheet-field">
            <span>TPM</span>
            <input
              name="auth-key-tpm"
              inputMode="numeric"
              autoComplete="off"
              value={accessForm.tpm}
              onChange={(event) => onAccessFormChange({ tpm: event.target.value })}
            />
          </label>
          <label className="sheet-field">
            <span>Cost / day USD</span>
            <input
              name="auth-key-cost-per-day"
              inputMode="decimal"
              autoComplete="off"
              value={accessForm.costPerDayUsd}
              onChange={(event) => onAccessFormChange({ costPerDayUsd: event.target.value })}
            />
          </label>
          <label className="sheet-field">
            <span>Expires at</span>
            <input
              name="auth-key-expires-at"
              type="datetime-local"
              value={accessForm.expiresAt}
              onChange={(event) => onAccessFormChange({ expiresAt: event.target.value })}
            />
          </label>
          <label className="detail-grid__row">
            <span>Budget enabled</span>
            <input
              type="checkbox"
              checked={accessForm.budgetEnabled}
              onChange={(event) => onAccessFormChange({ budgetEnabled: event.target.checked })}
            />
          </label>
          <label className="sheet-field">
            <span>Budget total USD</span>
            <input
              name="auth-key-budget-total"
              inputMode="decimal"
              autoComplete="off"
              value={accessForm.budgetTotalUsd}
              onChange={(event) => onAccessFormChange({ budgetTotalUsd: event.target.value })}
            />
          </label>
          <label className="sheet-field">
            <span>Budget period</span>
            <select
              value={accessForm.budgetPeriod}
              onChange={(event) => onAccessFormChange({ budgetPeriod: event.target.value as AccessPolicyFormState['budgetPeriod'] })}
            >
              <option value="daily">daily</option>
              <option value="monthly">monthly</option>
            </select>
          </label>
        </div>
      </section>

      {selectedAuthKey ? (
        <section className="sheet-section">
          <h3>Selected key posture</h3>
          <div className="detail-grid">
            <div className="detail-grid__row"><span>Key</span><strong>{selectedAuthKey.key_masked}</strong></div>
            <div className="detail-grid__row"><span>Tenant</span><strong>{selectedAuthKey.tenant_id ?? 'global'}</strong></div>
            <div className="detail-grid__row"><span>Model allowlist</span><strong>{selectedAuthKey.allowed_models.length || 'all'}</strong></div>
            <div className="detail-grid__row"><span>Credential allowlist</span><strong>{selectedAuthKey.allowed_credentials.length || 'all'}</strong></div>
          </div>
        </section>
      ) : null}

      <section className="sheet-section">
        <h3>Existing keys</h3>
        <div className="probe-list">
          {authKeys.length === 0 ? (
            <div className="probe-check">
              <span>Keys</span>
              <strong>None configured</strong>
            </div>
          ) : (
            authKeys.map((item) => (
              <div key={item.id} className={`probe-check ${item.id === selectedAuthKeyId ? 'probe-check--selected' : ''}`}>
                <span>{item.key_masked}</span>
                <strong>{item.name ?? 'unnamed'} · {item.tenant_id ?? 'global'}</strong>
                <button
                  type="button"
                  className="button button--ghost"
                  onClick={() => onSelectAuthKey(item.id)}
                >
                  Select
                </button>
              </div>
            ))
          )}
        </div>
      </section>
    </WorkbenchSheet>
  );
}
