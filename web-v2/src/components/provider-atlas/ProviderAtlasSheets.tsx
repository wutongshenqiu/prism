import { WorkbenchSheet } from '../WorkbenchSheet';
import { isManagedMode, profileKey, type AuthProfileFormState, type DeviceFlowState } from '../../lib/authProfileDraft';
import type {
  AuthProfileSummary,
  AuthProfilesRuntimeResponse,
  PresentationPreviewResponse,
  ProviderCapabilityEntry,
  ProviderDetail,
  ProviderHealthResult,
} from '../../types/backend';
import type { ProviderAtlasRow } from '../../types/controlPlane';
import type { ProviderEditorFormState, ProviderRegistryFormState } from './types';

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
  selectedCapabilities: ProviderCapabilityEntry | null;
  formState: ProviderEditorFormState;
  refreshingProfileId: string | null;
  onClose: () => void;
  onRunHealthCheck: () => void;
  onRunPresentationPreview: () => void;
  onSaveProvider: () => void;
  onFormStateChange: (patch: Partial<ProviderEditorFormState>) => void;
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
  selectedCapabilities,
  formState,
  refreshingProfileId,
  onClose,
  onRunHealthCheck,
  onRunPresentationPreview,
  onSaveProvider,
  onFormStateChange,
  onRefreshAuthProfile,
}: ProviderEditorSheetProps) {
  return (
    <WorkbenchSheet
      open={open}
      onClose={onClose}
      title="Provider editor"
      subtitle="Edit runtime-facing provider fields, run a real upstream health probe, and preview presentation mutations."
      actions={(
        <>
          <button type="button" className="button button--ghost" onClick={onRunHealthCheck}>
            Run health probe
          </button>
          <button type="button" className="button button--ghost" onClick={onRunPresentationPreview} disabled={previewing}>
            {previewing ? 'Previewing…' : 'Presentation preview'}
          </button>
          <button type="button" className="button button--primary" onClick={onSaveProvider} disabled={saving}>
            {saving ? 'Saving…' : 'Save provider'}
          </button>
        </>
      )}
    >
      {loadingDetail ? <div className="status-message">Loading provider detail…</div> : null}
      {actionStatus ? <div className="status-message status-message--success">{actionStatus}</div> : null}
      {actionError ? <div className="status-message status-message--danger">{actionError}</div> : null}

      {detail ? (
        <>
          <section className="sheet-section">
            <h3>Provider posture</h3>
            <div className="detail-grid">
              <div className="detail-grid__row"><span>Name</span><strong>{detail.name}</strong></div>
              <div className="detail-grid__row"><span>Format</span><strong>{detail.format}</strong></div>
              <div className="detail-grid__row"><span>Upstream</span><strong>{detail.upstream}</strong></div>
              <div className="detail-grid__row"><span>Auth profiles</span><strong>{detail.auth_profiles.length}</strong></div>
            </div>
          </section>

          <section className="sheet-section">
            <h3>Editable runtime fields</h3>
            <div className="sheet-form">
              <label className="sheet-field">
                <span>Base URL</span>
                <input
                  name="provider-base-url"
                  type="url"
                  autoComplete="url"
                  value={formState.baseUrl}
                  onChange={(event) => onFormStateChange({ baseUrl: event.target.value })}
                />
              </label>
              <label className="sheet-field">
                <span>Region</span>
                <input
                  name="provider-region"
                  autoComplete="off"
                  value={formState.region}
                  onChange={(event) => onFormStateChange({ region: event.target.value })}
                />
              </label>
              <label className="sheet-field">
                <span>Weight</span>
                <input
                  name="provider-weight"
                  inputMode="numeric"
                  autoComplete="off"
                  value={formState.weight}
                  onChange={(event) => onFormStateChange({ weight: event.target.value })}
                />
              </label>
              <label className="detail-grid__row">
                <span>Disabled</span>
                <input
                  type="checkbox"
                  checked={formState.disabled}
                  onChange={(event) => onFormStateChange({ disabled: event.target.checked })}
                />
              </label>
            </div>
          </section>

          <section className="sheet-section">
            <h3>Auth profiles</h3>
            <div className="probe-list">
              {detail.auth_profiles.length === 0 ? (
                <div className="probe-check">
                  <span>Profiles</span>
                  <strong>None configured</strong>
                </div>
              ) : (
                detail.auth_profiles.map((profile) => (
                  <div key={profile.qualified_name} className="probe-check">
                    <span>{profile.qualified_name}</span>
                    <strong>{profile.mode}</strong>
                    {profile.refresh_token_present ? (
                      <button
                        type="button"
                        className="button button--ghost"
                        onClick={() => onRefreshAuthProfile(detail.name, profile.id)}
                        disabled={refreshingProfileId === profileKey(detail.name, profile.id)}
                      >
                        {refreshingProfileId === profileKey(detail.name, profile.id) ? 'Refreshing…' : 'Refresh'}
                      </button>
                    ) : null}
                  </div>
                ))
              )}
            </div>
          </section>

          {runtimeInfo ? (
            <section className="sheet-section">
              <h3>Managed auth runtime</h3>
              <div className="detail-grid">
                <div className="detail-grid__row"><span>Storage dir</span><strong>{runtimeInfo.storage_dir ?? 'not configured'}</strong></div>
                <div className="detail-grid__row"><span>Codex auth file</span><strong>{runtimeInfo.codex_auth_file ?? 'not configured'}</strong></div>
                <div className="detail-grid__row"><span>Proxy URL</span><strong>{runtimeInfo.proxy_url ?? 'none'}</strong></div>
              </div>
            </section>
          ) : null}

          {selectedCapabilities ? (
            <section className="sheet-section">
              <h3>Capability snapshot</h3>
              <div className="detail-grid">
                <div className="detail-grid__row"><span>Probe status</span><strong>{selectedCapabilities.probe_status}</strong></div>
                <div className="detail-grid__row"><span>Presentation</span><strong>{selectedCapabilities.presentation_profile}</strong></div>
                <div className="detail-grid__row"><span>Models</span><strong>{selectedCapabilities.models.length}</strong></div>
                <div className="detail-grid__row"><span>Wire API</span><strong>{selectedCapabilities.wire_api}</strong></div>
              </div>
            </section>
          ) : null}

          {health ? (
            <section className="sheet-section">
              <h3>Health probe</h3>
              <div className="detail-grid">
                <div className="detail-grid__row"><span>Status</span><strong>{health.status}</strong></div>
                <div className="detail-grid__row"><span>Checked at</span><strong>{health.checked_at}</strong></div>
                <div className="detail-grid__row"><span>Latency</span><strong>{health.latency_ms} ms</strong></div>
              </div>
              <div className="probe-list">
                {health.checks.map((check) => (
                  <div key={check.capability} className="probe-check">
                    <span>{check.capability}</span>
                    <strong>{check.status}</strong>
                  </div>
                ))}
              </div>
            </section>
          ) : null}

          {preview ? (
            <section className="sheet-section">
              <h3>Presentation preview</h3>
              <div className="detail-grid">
                <div className="detail-grid__row"><span>Profile</span><strong>{preview.profile}</strong></div>
                <div className="detail-grid__row"><span>Activated</span><strong>{preview.activated ? 'yes' : 'no'}</strong></div>
                <div className="detail-grid__row"><span>Protected headers blocked</span><strong>{preview.protected_headers_blocked.length}</strong></div>
                <div className="detail-grid__row"><span>Mutations</span><strong>{preview.body_mutations.length}</strong></div>
              </div>
              <div className="probe-list">
                {preview.body_mutations.map((mutation) => (
                  <div key={`${mutation.kind}-${mutation.reason ?? 'none'}`} className="probe-check">
                    <span>{mutation.kind}</span>
                    <strong>{mutation.applied ? 'applied' : mutation.reason ?? 'skipped'}</strong>
                  </div>
                ))}
              </div>
            </section>
          ) : null}
        </>
      ) : null}
    </WorkbenchSheet>
  );
}

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
  return (
    <WorkbenchSheet
      open={open}
      onClose={onClose}
      title="Provider registry workbench"
      subtitle="Create disabled providers, fetch model inventories, and remove obsolete runtime entities without leaving the atlas."
      actions={(
        <>
          <button type="button" className="button button--ghost" onClick={onFetchModels} disabled={registryLoading}>
            {registryLoading ? 'Working…' : 'Fetch models'}
          </button>
          <button type="button" className="button button--ghost" onClick={onDeleteSelectedProvider} disabled={registryLoading || !selectedProvider}>
            Delete selected
          </button>
          <button type="button" className="button button--primary" onClick={onCreateProvider} disabled={registryLoading}>
            {registryLoading ? 'Saving…' : 'Create provider'}
          </button>
        </>
      )}
    >
      {registryStatus ? <div className="status-message status-message--success">{registryStatus}</div> : null}
      {registryError ? <div className="status-message status-message--danger">{registryError}</div> : null}

      <section className="sheet-section">
        <h3>New provider draft</h3>
        <form
          className="sheet-form"
          onSubmit={(event) => {
            event.preventDefault();
            onCreateProvider();
          }}
        >
          <label className="sheet-field">
            <span>Name</span>
            <input
              name="provider-name"
              autoComplete="organization"
              value={registryForm.name}
              onChange={(event) => onRegistryFormChange({ name: event.target.value })}
            />
          </label>
          <label className="sheet-field">
            <span>Format</span>
            <select value={registryForm.format} onChange={(event) => onRegistryFormChange({ format: event.target.value as ProviderRegistryFormState['format'] })}>
              <option value="openai">openai</option>
              <option value="claude">claude</option>
              <option value="gemini">gemini</option>
            </select>
          </label>
          <label className="sheet-field">
            <span>Upstream</span>
            <input
              name="provider-upstream"
              autoComplete="off"
              value={registryForm.upstream}
              onChange={(event) => onRegistryFormChange({ upstream: event.target.value })}
            />
          </label>
          <label className="sheet-field">
            <span>API key</span>
            <input
              name="provider-api-key"
              type="password"
              autoComplete="new-password"
              value={registryForm.apiKey}
              onChange={(event) => onRegistryFormChange({ apiKey: event.target.value })}
            />
          </label>
          <label className="sheet-field">
            <span>Base URL</span>
            <input
              name="registry-base-url"
              type="url"
              autoComplete="url"
              value={registryForm.baseUrl}
              onChange={(event) => onRegistryFormChange({ baseUrl: event.target.value })}
            />
          </label>
          <label className="sheet-field">
            <span>Models</span>
            <input
              name="provider-models"
              autoComplete="off"
              value={registryForm.models}
              onChange={(event) => onRegistryFormChange({ models: event.target.value })}
            />
          </label>
          <label className="detail-grid__row">
            <span>Disabled</span>
            <input
              type="checkbox"
              checked={registryForm.disabled}
              onChange={(event) => onRegistryFormChange({ disabled: event.target.checked })}
            />
          </label>
        </form>
      </section>

      <section className="sheet-section">
        <h3>Selected provider</h3>
        <div className="detail-grid">
          <div className="detail-grid__row"><span>Name</span><strong>{selectedProvider ?? 'none selected'}</strong></div>
          <div className="detail-grid__row"><span>Status</span><strong>{selectedRow?.status ?? 'n/a'}</strong></div>
          <div className="detail-grid__row"><span>Auth posture</span><strong>{selectedRow?.auth ?? 'n/a'}</strong></div>
          <div className="detail-grid__row"><span>Coverage</span><strong>{selectedProbeStatus ?? 'n/a'}</strong></div>
        </div>
      </section>
    </WorkbenchSheet>
  );
}

interface AuthProfileWorkbenchSheetProps {
  open: boolean;
  authLoading: boolean;
  authStatus: string | null;
  authError: string | null;
  authSaving: boolean;
  authEditorMode: 'create' | 'edit';
  runtimeInfo: AuthProfilesRuntimeResponse | null;
  providers: Array<{ provider: string }>;
  authForm: AuthProfileFormState;
  selectedAuthProfile: AuthProfileSummary | null;
  selectedProfiles: AuthProfileSummary[];
  selectedAuthProfileId: string | null;
  selectedAuthProfileMode: string;
  connectSecret: string;
  importPath: string;
  deviceFlow: DeviceFlowState | null;
  importingProfileId: string | null;
  refreshingProfileId: string | null;
  connectingProfileId: string | null;
  onClose: () => void;
  onStartNewDraft: () => void;
  onImportSelectedProfile: () => void;
  onStartBrowserOauth: () => void;
  onStartDeviceFlow: () => void;
  onRefreshSelectedProfile: () => void;
  onDeleteSelectedProfile: () => void;
  onSaveAuthProfile: () => void;
  onConnectSelectedProfile: () => void;
  onAuthFormChange: (patch: Partial<AuthProfileFormState>) => void;
  onConnectSecretChange: (value: string) => void;
  onImportPathChange: (value: string) => void;
  onSelectExistingProfile: (profileKey: string) => void;
}

export function AuthProfileWorkbenchSheet({
  open,
  authLoading,
  authStatus,
  authError,
  authSaving,
  authEditorMode,
  runtimeInfo,
  providers,
  authForm,
  selectedAuthProfile,
  selectedProfiles,
  selectedAuthProfileId,
  selectedAuthProfileMode,
  connectSecret,
  importPath,
  deviceFlow,
  importingProfileId,
  refreshingProfileId,
  connectingProfileId,
  onClose,
  onStartNewDraft,
  onImportSelectedProfile,
  onStartBrowserOauth,
  onStartDeviceFlow,
  onRefreshSelectedProfile,
  onDeleteSelectedProfile,
  onSaveAuthProfile,
  onConnectSelectedProfile,
  onAuthFormChange,
  onConnectSecretChange,
  onImportPathChange,
  onSelectExistingProfile,
}: AuthProfileWorkbenchSheetProps) {
  return (
    <WorkbenchSheet
      open={open}
      onClose={onClose}
      title="Auth profile workbench"
      subtitle="Managed auth should be operated as first-class provider identity, not hidden behind provider config blobs."
      actions={(
        <>
          <button type="button" className="button button--ghost" onClick={onStartNewDraft}>
            New draft
          </button>
          <button
            type="button"
            className="button button--ghost"
            onClick={onImportSelectedProfile}
            disabled={!selectedAuthProfile || importingProfileId !== null}
          >
            {importingProfileId ? 'Importing…' : 'Import local'}
          </button>
          <button
            type="button"
            className="button button--ghost"
            onClick={onStartBrowserOauth}
            disabled={!selectedAuthProfile || selectedAuthProfileMode !== 'codex-oauth' || connectingProfileId !== null}
          >
            {connectingProfileId ? 'Connecting…' : 'Browser OAuth'}
          </button>
          <button
            type="button"
            className="button button--ghost"
            onClick={onStartDeviceFlow}
            disabled={!selectedAuthProfile || selectedAuthProfileMode !== 'codex-oauth' || connectingProfileId !== null}
          >
            {deviceFlow ? 'Device active' : 'Device flow'}
          </button>
          <button
            type="button"
            className="button button--ghost"
            onClick={onRefreshSelectedProfile}
            disabled={!selectedAuthProfile || refreshingProfileId !== null}
          >
            {refreshingProfileId ? 'Refreshing…' : 'Refresh selected'}
          </button>
          <button type="button" className="button button--ghost" onClick={onDeleteSelectedProfile} disabled={!selectedAuthProfile}>
            Delete selected
          </button>
          <button type="button" className="button button--primary" onClick={onSaveAuthProfile} disabled={authSaving}>
            {authSaving ? 'Saving…' : authEditorMode === 'edit' ? 'Save profile' : 'Create profile'}
          </button>
        </>
      )}
    >
      {authLoading ? <div className="status-message">Loading auth profiles…</div> : null}
      {authStatus ? <div className="status-message status-message--success">{authStatus}</div> : null}
      {authError ? <div className="status-message status-message--danger">{authError}</div> : null}

      {runtimeInfo ? (
        <section className="sheet-section">
          <h3>Managed auth runtime</h3>
          <div className="detail-grid">
            <div className="detail-grid__row"><span>Storage dir</span><strong>{runtimeInfo.storage_dir ?? 'not configured'}</strong></div>
            <div className="detail-grid__row"><span>Codex auth file</span><strong>{runtimeInfo.codex_auth_file ?? 'not configured'}</strong></div>
            <div className="detail-grid__row"><span>Proxy URL</span><strong>{runtimeInfo.proxy_url ?? 'none'}</strong></div>
          </div>
        </section>
      ) : null}

      <section className="sheet-section">
        <h3>{authEditorMode === 'edit' ? 'Edit profile' : 'Create profile'}</h3>
        <form
          className="sheet-form"
          onSubmit={(event) => {
            event.preventDefault();
            onSaveAuthProfile();
          }}
        >
          <label className="sheet-field">
            <span>Provider</span>
            <select value={authForm.provider} onChange={(event) => onAuthFormChange({ provider: event.target.value })}>
              {providers.map((provider) => (
                <option key={provider.provider} value={provider.provider}>{provider.provider}</option>
              ))}
            </select>
          </label>
          <label className="sheet-field">
            <span>Profile id</span>
            <input
              name="auth-profile-id"
              autoComplete="username"
              value={authForm.id}
              onChange={(event) => onAuthFormChange({ id: event.target.value })}
            />
          </label>
          <label className="sheet-field">
            <span>Mode</span>
            <select value={authForm.mode} onChange={(event) => onAuthFormChange({ mode: event.target.value })}>
              <option value="api-key">api-key</option>
              <option value="bearer-token">bearer-token</option>
              <option value="codex-oauth">codex-oauth</option>
              <option value="anthropic-claude-subscription">anthropic-claude-subscription</option>
            </select>
          </label>
          <label className="sheet-field">
            <span>{isManagedMode(authForm.mode) ? 'Secret (optional on create)' : 'Secret'}</span>
            <input
              name="auth-profile-secret"
              type="password"
              autoComplete="new-password"
              value={authForm.secret}
              onChange={(event) => onAuthFormChange({ secret: event.target.value })}
            />
          </label>
          <label className="sheet-field">
            <span>Weight</span>
            <input
              name="auth-profile-weight"
              inputMode="numeric"
              autoComplete="off"
              value={authForm.weight}
              onChange={(event) => onAuthFormChange({ weight: event.target.value })}
            />
          </label>
          <label className="sheet-field">
            <span>Region</span>
            <input
              name="auth-profile-region"
              autoComplete="off"
              value={authForm.region}
              onChange={(event) => onAuthFormChange({ region: event.target.value })}
            />
          </label>
          <label className="sheet-field">
            <span>Prefix</span>
            <input
              name="auth-profile-prefix"
              autoComplete="off"
              value={authForm.prefix}
              onChange={(event) => onAuthFormChange({ prefix: event.target.value })}
            />
          </label>
          <label className="detail-grid__row">
            <span>Disabled</span>
            <input
              type="checkbox"
              checked={authForm.disabled}
              onChange={(event) => onAuthFormChange({ disabled: event.target.checked })}
            />
          </label>
        </form>
      </section>

      {selectedAuthProfile ? (
        <section className="sheet-section">
          <h3>Selected profile posture</h3>
          <div className="detail-grid">
            <div className="detail-grid__row"><span>Profile</span><strong>{selectedAuthProfile.qualified_name}</strong></div>
            <div className="detail-grid__row"><span>Mode</span><strong>{selectedAuthProfile.mode}</strong></div>
            <div className="detail-grid__row"><span>Connected</span><strong>{selectedAuthProfile.connected ? 'yes' : 'no'}</strong></div>
            <div className="detail-grid__row"><span>Account</span><strong>{selectedAuthProfile.email ?? selectedAuthProfile.account_id ?? 'unknown'}</strong></div>
          </div>

          {selectedAuthProfile.mode === 'anthropic-claude-subscription' ? (
            <div className="sheet-form">
              <label className="sheet-field">
                <span>Subscription token</span>
                <input
                  name="auth-profile-connect-secret"
                  type="password"
                  autoComplete="new-password"
                  value={connectSecret}
                  onChange={(event) => onConnectSecretChange(event.target.value)}
                />
              </label>
              <button
                type="button"
                className="button button--secondary"
                onClick={onConnectSelectedProfile}
                disabled={connectingProfileId === profileKey(selectedAuthProfile.provider, selectedAuthProfile.id)}
              >
                {connectingProfileId === profileKey(selectedAuthProfile.provider, selectedAuthProfile.id) ? 'Connecting…' : 'Connect secret'}
              </button>
            </div>
          ) : null}

          {selectedAuthProfile.mode === 'codex-oauth' ? (
            <>
              <div className="sheet-form">
                <label className="sheet-field">
                  <span>Import path</span>
                  <input
                    name="auth-profile-import-path"
                    autoComplete="off"
                    value={importPath}
                    onChange={(event) => onImportPathChange(event.target.value)}
                  />
                </label>
              </div>
              {deviceFlow ? (
                <div className="status-message status-message--warning">
                  Device flow active. Visit <strong>{deviceFlow.verification_url}</strong> and enter code <strong>{deviceFlow.user_code}</strong>.
                </div>
              ) : null}
            </>
          ) : null}
        </section>
      ) : null}

      <section className="sheet-section">
        <h3>Existing profiles</h3>
        <div className="probe-list">
          {selectedProfiles.length === 0 ? (
            <div className="probe-check">
              <span>Profiles</span>
              <strong>None configured for this provider</strong>
            </div>
          ) : (
            selectedProfiles.map((profile) => {
              const currentKey = profileKey(profile.provider, profile.id);
              return (
                <div key={currentKey} className={`probe-check ${selectedAuthProfileId === currentKey ? 'probe-check--selected' : ''}`}>
                  <span>{profile.qualified_name}</span>
                  <strong>{profile.mode} · {profile.connected ? 'connected' : 'disconnected'}</strong>
                  <button
                    type="button"
                    className="button button--ghost"
                    onClick={() => onSelectExistingProfile(currentKey)}
                  >
                    Select
                  </button>
                </div>
              );
            })
          )}
        </div>
      </section>
    </WorkbenchSheet>
  );
}
