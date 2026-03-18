import { WorkbenchSheet } from '../WorkbenchSheet';
import { useI18n } from '../../i18n';
import { presentAuthMode } from '../../lib/operatorPresentation';
import {
  isManagedMode,
  profileKey,
  type AuthProfileFormState,
  type DeviceFlowState,
} from '../../lib/authProfileDraft';
import type { AuthProfileSummary, AuthProfilesRuntimeResponse } from '../../types/backend';

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
  const { t, formatDateTime } = useI18n();

  return (
    <WorkbenchSheet
      open={open}
      onClose={onClose}
      title={t('providerAtlas.authWorkbench.title')}
      subtitle={t('providerAtlas.authWorkbench.subtitle')}
      actions={(
        <>
          <button type="button" className="button button--ghost" onClick={onStartNewDraft}>
            {t('providerAtlas.authWorkbench.newDraft')}
          </button>
          <button type="button" className="button button--primary" onClick={onSaveAuthProfile} disabled={authSaving}>
            {authSaving
              ? t('providerAtlas.authWorkbench.saving')
              : authEditorMode === 'edit'
                ? t('providerAtlas.authWorkbench.saveProfile')
                : t('providerAtlas.authWorkbench.createProfile')}
          </button>
        </>
      )}
    >
      {authLoading ? <div className="status-message">{t('providerAtlas.authWorkbench.loadingProfiles')}</div> : null}
      {authStatus ? <div className="status-message status-message--success">{authStatus}</div> : null}
      {authError ? <div className="status-message status-message--danger">{authError}</div> : null}

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

      <section className="sheet-section">
        <h3>{authEditorMode === 'edit' ? t('providerAtlas.authWorkbench.editProfile') : t('providerAtlas.authWorkbench.createProfileHeading')}</h3>
        <form
          className="sheet-form"
          onSubmit={(event) => {
            event.preventDefault();
            onSaveAuthProfile();
          }}
        >
          <label className="sheet-field">
            <span>{t('common.provider')}</span>
            <select value={authForm.provider} onChange={(event) => onAuthFormChange({ provider: event.target.value })}>
              {providers.map((provider) => (
                <option key={provider.provider} value={provider.provider}>{provider.provider}</option>
              ))}
            </select>
          </label>
          <label className="sheet-field">
            <span>{t('providerAtlas.authWorkbench.profileId')}</span>
            <input
              name="auth-profile-id"
              autoComplete="username"
              value={authForm.id}
              onChange={(event) => onAuthFormChange({ id: event.target.value })}
            />
          </label>
          <label className="sheet-field">
            <span>{t('common.mode')}</span>
            <select value={authForm.mode} onChange={(event) => onAuthFormChange({ mode: event.target.value })}>
              <option value="api-key">{presentAuthMode('api-key', t)}</option>
              <option value="bearer-token">{presentAuthMode('bearer-token', t)}</option>
              <option value="codex-oauth">{presentAuthMode('codex-oauth', t)}</option>
              <option value="anthropic-claude-subscription">{presentAuthMode('anthropic-claude-subscription', t)}</option>
            </select>
          </label>
          <label className="sheet-field">
            <span>{isManagedMode(authForm.mode) ? t('providerAtlas.authWorkbench.secretOptional') : t('providerAtlas.authWorkbench.secret')}</span>
            <input
              name="auth-profile-secret"
              type="password"
              autoComplete="new-password"
              value={authForm.secret}
              onChange={(event) => onAuthFormChange({ secret: event.target.value })}
            />
          </label>
          <label className="sheet-field">
            <span>{t('common.weight')}</span>
            <input
              name="auth-profile-weight"
              inputMode="numeric"
              autoComplete="off"
              value={authForm.weight}
              onChange={(event) => onAuthFormChange({ weight: event.target.value })}
            />
          </label>
          <label className="sheet-field">
            <span>{t('common.region')}</span>
            <input
              name="auth-profile-region"
              autoComplete="off"
              value={authForm.region}
              onChange={(event) => onAuthFormChange({ region: event.target.value })}
            />
          </label>
          <label className="sheet-field">
            <span>{t('providerAtlas.authWorkbench.prefix')}</span>
            <input
              name="auth-profile-prefix"
              autoComplete="off"
              value={authForm.prefix}
              onChange={(event) => onAuthFormChange({ prefix: event.target.value })}
            />
          </label>
          <label className="detail-grid__row">
            <span>{t('common.disabled')}</span>
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
          <h3>{t('providerAtlas.authWorkbench.selectedProfilePosture')}</h3>
          <div className="detail-grid">
            <div className="detail-grid__row"><span>{t('common.profile')}</span><strong>{selectedAuthProfile.qualified_name}</strong></div>
            <div className="detail-grid__row"><span>{t('common.mode')}</span><strong>{presentAuthMode(selectedAuthProfile.mode, t)}</strong></div>
            <div className="detail-grid__row"><span>{t('providerAtlas.authWorkbench.connected')}</span><strong>{selectedAuthProfile.connected ? t('common.yes') : t('common.no')}</strong></div>
            <div className="detail-grid__row"><span>{t('providerAtlas.authWorkbench.account')}</span><strong>{selectedAuthProfile.email ?? selectedAuthProfile.account_id ?? t('common.unknown')}</strong></div>
            {selectedAuthProfile.expires_at ? (
              <div className="detail-grid__row"><span>{t('providerAtlas.authWorkbench.expiresAt')}</span><strong>{formatDateTime(selectedAuthProfile.expires_at)}</strong></div>
            ) : null}
          </div>

          <div className="sheet-form">
            <h4>{t('providerAtlas.authWorkbench.selectedActions')}</h4>
            <div className="inline-actions inline-actions--wrap">
              <button
                type="button"
                className="button button--ghost"
                onClick={onImportSelectedProfile}
                disabled={importingProfileId !== null}
              >
                {importingProfileId ? t('providerAtlas.authWorkbench.importing') : t('providerAtlas.authWorkbench.importLocal')}
              </button>
              <button
                type="button"
                className="button button--ghost"
                onClick={onRefreshSelectedProfile}
                disabled={refreshingProfileId !== null}
              >
                {refreshingProfileId ? t('providerAtlas.authWorkbench.refreshing') : t('providerAtlas.authWorkbench.refreshSelected')}
              </button>
              <button type="button" className="button button--ghost" onClick={onDeleteSelectedProfile}>
                {t('providerAtlas.authWorkbench.deleteSelected')}
              </button>
              <button
                type="button"
                className="button button--ghost"
                onClick={onStartBrowserOauth}
                disabled={selectedAuthProfileMode !== 'codex-oauth' || connectingProfileId !== null}
              >
                {connectingProfileId ? t('providerAtlas.authWorkbench.connecting') : t('providerAtlas.authWorkbench.browserOauth')}
              </button>
              <button
                type="button"
                className="button button--ghost"
                onClick={onStartDeviceFlow}
                disabled={selectedAuthProfileMode !== 'codex-oauth' || connectingProfileId !== null}
              >
                {deviceFlow ? t('providerAtlas.authWorkbench.deviceActive') : t('providerAtlas.authWorkbench.deviceFlow')}
              </button>
            </div>
          </div>

          {selectedAuthProfile.mode === 'anthropic-claude-subscription' ? (
            <div className="sheet-form">
              <label className="sheet-field">
                <span>{t('providerAtlas.authWorkbench.subscriptionToken')}</span>
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
                {connectingProfileId === profileKey(selectedAuthProfile.provider, selectedAuthProfile.id)
                  ? t('providerAtlas.authWorkbench.connecting')
                  : t('providerAtlas.authWorkbench.connectSecret')}
              </button>
            </div>
          ) : null}

          {selectedAuthProfile.mode === 'codex-oauth' ? (
            <>
              <div className="sheet-form">
                <label className="sheet-field">
                  <span>{t('providerAtlas.authWorkbench.importPath')}</span>
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
                  {t('providerAtlas.authWorkbench.deviceFlowActive', {
                    verificationUrl: deviceFlow.verification_url,
                    userCode: deviceFlow.user_code,
                  })}
                </div>
              ) : null}
            </>
          ) : null}
        </section>
      ) : null}

      <section className="sheet-section">
        <h3>{t('providerAtlas.authWorkbench.existingProfiles')}</h3>
        <div className="probe-list">
          {selectedProfiles.length === 0 ? (
            <div className="probe-check">
              <span>{t('providerAtlas.authWorkbench.profilePlural')}</span>
              <strong>{t('providerAtlas.authWorkbench.noneConfigured')}</strong>
            </div>
          ) : (
            selectedProfiles.map((profile) => {
              const currentKey = profileKey(profile.provider, profile.id);
              return (
                <div key={currentKey} className={`probe-check ${selectedAuthProfileId === currentKey ? 'probe-check--selected' : ''}`}>
                  <span>{profile.qualified_name}</span>
                  <strong>{presentAuthMode(profile.mode, t)} · {profile.connected ? t('common.connected') : t('providerAtlas.authWorkbench.disconnected')}</strong>
                  <button
                    type="button"
                    className="button button--ghost"
                    onClick={() => onSelectExistingProfile(currentKey)}
                  >
                    {t('common.select')}
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
