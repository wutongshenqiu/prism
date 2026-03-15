import { useEffect, useMemo, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { KeyRound, Plus, RefreshCw, Pencil, Trash2, Link as LinkIcon, X, Laptop, Smartphone, Globe } from 'lucide-react';
import { authProfilesApi, providersApi } from '../services/api';
import type {
  AuthMode,
  AuthProfile,
  AuthProfileUpsertRequest,
  CodexDeviceStartResponse,
  ManagedAuthRuntimeStatus,
  Provider,
} from '../types';
import { extractApiErrorMessage } from '../utils/apiError';
import StatusBadge from '../components/StatusBadge';

interface FormState {
  provider: string;
  id: string;
  mode: AuthMode;
  secret: string;
  disabled: boolean;
  weight: number;
  region: string;
  prefix: string;
}

interface NoticeState {
  type: 'success' | 'error' | 'warning';
  message: string;
}

interface CodexDeviceState extends CodexDeviceStartResponse {
  status: 'idle' | 'pending';
}

const emptyForm: FormState = {
  provider: '',
  id: '',
  mode: 'api-key',
  secret: '',
  disabled: false,
  weight: 1,
  region: '',
  prefix: '',
};

function isManagedMode(mode: AuthMode) {
  return mode === 'codex-oauth' || mode === 'anthropic-claude-subscription';
}

function modeLabel(mode: AuthMode) {
  switch (mode) {
    case 'api-key':
      return 'API key';
    case 'bearer-token':
      return 'Bearer token';
    case 'codex-oauth':
      return 'Codex OAuth';
    case 'anthropic-claude-subscription':
      return 'Claude Subscription';
    default:
      return mode;
  }
}

export default function AuthProfiles() {
  const [profiles, setProfiles] = useState<AuthProfile[]>([]);
  const [providers, setProviders] = useState<Provider[]>([]);
  const [managedAuthRuntime, setManagedAuthRuntime] = useState<ManagedAuthRuntimeStatus | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [showModal, setShowModal] = useState(false);
  const [editing, setEditing] = useState<AuthProfile | null>(null);
  const [form, setForm] = useState<FormState>(emptyForm);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState<NoticeState | null>(null);
  const [saving, setSaving] = useState(false);
  const [connecting, setConnecting] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState<string | null>(null);
  const [connectProfile, setConnectProfile] = useState<AuthProfile | null>(null);
  const [connectSecret, setConnectSecret] = useState('');
  const [connectImportPath, setConnectImportPath] = useState('');
  const [connectError, setConnectError] = useState('');
  const [codexDevice, setCodexDevice] = useState<CodexDeviceState | null>(null);
  const [searchParams, setSearchParams] = useSearchParams();
  const navigate = useNavigate();

  const providerFilter = searchParams.get('provider') ?? '';
  const focusedProfile = searchParams.get('profile') ?? '';
  const oauthStatus = searchParams.get('oauth');
  const providersByName = useMemo(
    () => new Map(providers.map((provider) => [provider.name, provider])),
    [providers],
  );

  const allowedModesForProvider = (providerName: string): AuthMode[] => {
    const provider = providersByName.get(providerName);
    switch (provider?.upstream) {
      case 'codex':
        return ['codex-oauth'];
      case 'claude':
        return ['api-key', 'bearer-token', 'anthropic-claude-subscription'];
      default:
        return ['api-key', 'bearer-token'];
    }
  };

  const normalizeModeForProvider = (providerName: string, mode: AuthMode): AuthMode => {
    const allowed = allowedModesForProvider(providerName);
    return allowed.includes(mode) ? mode : allowed[0] ?? 'api-key';
  };

  const fetchData = async () => {
    setIsLoading(true);
    try {
      const [profilesResp, providersResp, runtimeResp] = await Promise.all([
        authProfilesApi.list(),
        providersApi.list(),
        authProfilesApi.runtime(),
      ]);
      setProfiles(profilesResp.data);
      setProviders(providersResp.data);
      setManagedAuthRuntime(runtimeResp);
    } catch (err) {
      setNotice({
        type: 'error',
        message: extractApiErrorMessage(err, 'Failed to load auth profiles'),
      });
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    void fetchData();
  }, []);

  useEffect(() => {
    if (!oauthStatus) return;
    setNotice({
      type: oauthStatus === 'success' ? 'success' : 'error',
      message:
        oauthStatus === 'success'
          ? `OAuth login completed for ${focusedProfile || 'auth profile'}.`
          : 'OAuth login failed.',
    });
    const next = new URLSearchParams(searchParams);
    next.delete('oauth');
    next.delete('profile');
    setSearchParams(next, { replace: true });
  }, [focusedProfile, oauthStatus, searchParams, setSearchParams]);

  const filteredProfiles = useMemo(() => {
    return profiles.filter((profile) =>
      !providerFilter || profile.provider === providerFilter
    );
  }, [profiles, providerFilter]);
  const defaultCodexAuthFile = managedAuthRuntime?.codex_auth_file ?? '~/.codex/auth.json';
  const authEgressLabel = managedAuthRuntime?.proxy_url || 'direct';

  const openCreate = () => {
    const provider = providerFilter || providers[0]?.name || '';
    setEditing(null);
    setForm({ ...emptyForm, provider, mode: normalizeModeForProvider(provider, emptyForm.mode) });
    setError('');
    setShowModal(true);
  };

  const openEdit = (profile: AuthProfile) => {
    setEditing(profile);
    setForm({
      provider: profile.provider,
      id: profile.id,
      mode: profile.mode,
      secret: '',
      disabled: profile.disabled,
      weight: profile.weight,
      region: profile.region ?? '',
      prefix: profile.prefix ?? '',
    });
    setError('');
    setShowModal(true);
  };

  const closeModal = () => {
    setShowModal(false);
    setEditing(null);
    setForm(emptyForm);
    setError('');
  };

  const closeConnectModal = () => {
    setConnectProfile(null);
    setConnectSecret('');
    setConnectImportPath('');
    setConnectError('');
    setCodexDevice(null);
    setConnecting(null);
  };

  const handleSave = async () => {
    if (!form.provider.trim() || !form.id.trim()) {
      setError('Provider and profile id are required');
      return;
    }
    if (!isManagedMode(form.mode) && !editing && !form.secret.trim()) {
      setError('Secret is required for API key and bearer token auth profiles');
      return;
    }

    const payload: AuthProfileUpsertRequest = {
      mode: form.mode,
      secret:
        isManagedMode(form.mode)
          ? undefined
          : (form.secret.trim() || (editing ? undefined : null)),
      disabled: form.disabled,
      weight: form.weight,
      region: form.region.trim() || null,
      prefix: form.prefix.trim() || null,
    };

    setSaving(true);
    setError('');

    try {
      if (editing) {
        await authProfilesApi.replace(editing.provider, editing.id, payload);
        setNotice({ type: 'success', message: `Auth profile "${editing.qualified_name}" updated.` });
      } else {
        await authProfilesApi.create({
          provider: form.provider,
          id: form.id,
          ...payload,
        });
        setNotice({ type: 'success', message: `Auth profile "${form.provider}/${form.id}" created.` });
      }
      closeModal();
      await fetchData();
    } catch (err) {
      setError(extractApiErrorMessage(err, 'Failed to save auth profile'));
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async (profile: AuthProfile) => {
    if (!window.confirm(`Delete auth profile "${profile.qualified_name}"?`)) {
      return;
    }
    try {
      await authProfilesApi.delete(profile.provider, profile.id);
      setNotice({ type: 'success', message: `Auth profile "${profile.qualified_name}" deleted.` });
      await fetchData();
    } catch (err) {
      setNotice({
        type: 'error',
        message: extractApiErrorMessage(err, 'Failed to delete auth profile'),
      });
    }
  };

  const handleConnect = async (profile: AuthProfile) => {
    setConnectProfile(profile);
    setConnectSecret('');
    setConnectImportPath(managedAuthRuntime?.codex_auth_file ?? '');
    setConnectError('');
    setCodexDevice(null);
  };

  const handleConnectToken = async () => {
    if (!connectProfile) return;
    if (!connectSecret.trim()) {
      setConnectError('Token is required');
      return;
    }

    setConnecting(connectProfile.qualified_name);
    setConnectError('');
    try {
      await authProfilesApi.connect(connectProfile.provider, connectProfile.id, {
        secret: connectSecret.trim(),
      });
      setNotice({
        type: 'success',
        message: `Auth profile "${connectProfile.qualified_name}" connected.`,
      });
      closeConnectModal();
      await fetchData();
    } catch (err) {
      setConnectError(extractApiErrorMessage(err, 'Failed to connect auth profile'));
    } finally {
      setConnecting(null);
    }
  };

  const handleImportLocalCodex = async () => {
    if (!connectProfile) return;
    setConnecting(connectProfile.qualified_name);
    setConnectError('');
    try {
      const requestedPath = connectImportPath.trim();
      await authProfilesApi.importLocal(connectProfile.provider, connectProfile.id, {
        path: requestedPath || undefined,
      });
      setNotice({
        type: 'success',
        message: requestedPath
          ? `Imported server-local Codex auth for "${connectProfile.qualified_name}" from ${requestedPath}.`
          : `Imported server-local Codex auth for "${connectProfile.qualified_name}".`,
      });
      closeConnectModal();
      await fetchData();
    } catch (err) {
      setConnectError(extractApiErrorMessage(err, 'Failed to import local auth.json'));
    } finally {
      setConnecting(null);
    }
  };

  const handleStartBrowserOauth = async () => {
    if (!connectProfile) return;
    setConnecting(connectProfile.qualified_name);
    try {
      const redirectUri = `${window.location.origin}/auth-profiles/callback`;
      const start = await authProfilesApi.startCodexOauth({
        provider: connectProfile.provider,
        profile_id: connectProfile.id,
        redirect_uri: redirectUri,
      });
      window.location.assign(start.auth_url);
    } catch (err) {
      setConnectError(extractApiErrorMessage(err, 'Failed to start OAuth login'));
      setConnecting(null);
    }
  };

  const handleStartDeviceFlow = async () => {
    if (!connectProfile) return;
    setConnecting(connectProfile.qualified_name);
    setConnectError('');
    try {
      const device = await authProfilesApi.startCodexDevice({
        provider: connectProfile.provider,
        profile_id: connectProfile.id,
      });
      setCodexDevice({ ...device, status: 'pending' });
    } catch (err) {
      setConnectError(extractApiErrorMessage(err, 'Failed to start device flow'));
    } finally {
      setConnecting(null);
    }
  };

  const handleRefresh = async (profile: AuthProfile) => {
    setRefreshing(profile.qualified_name);
    try {
      await authProfilesApi.refresh(profile.provider, profile.id);
      setNotice({ type: 'success', message: `Auth profile "${profile.qualified_name}" refreshed.` });
      await fetchData();
    } catch (err) {
      setNotice({
        type: 'error',
        message: extractApiErrorMessage(err, 'Failed to refresh auth profile'),
      });
    } finally {
      setRefreshing(null);
    }
  };

  useEffect(() => {
    if (!connectProfile || connectProfile.mode !== 'codex-oauth' || !codexDevice) {
      return;
    }

    let cancelled = false;
    const interval = window.setInterval(() => {
      if (cancelled) return;
      void authProfilesApi.pollCodexDevice(codexDevice.state)
        .then(async (result) => {
          if (cancelled || result.status !== 'completed') return;
          setNotice({
            type: 'success',
            message: `Auth profile "${connectProfile.qualified_name}" connected via device flow.`,
          });
          closeConnectModal();
          await fetchData();
        })
        .catch((err) => {
          if (cancelled) return;
          setConnectError(extractApiErrorMessage(err, 'Device flow polling failed'));
        });
    }, Math.max(codexDevice.interval_secs, 2) * 1000);

    return () => {
      cancelled = true;
      window.clearInterval(interval);
    };
  }, [codexDevice, connectProfile]);

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h2>Auth Profiles</h2>
          <p className="page-subtitle">Manage provider credentials, OAuth sessions, and routing identities.</p>
        </div>
        <div className="page-header-actions" style={{ display: 'flex', gap: 12 }}>
          <select
            value={providerFilter}
            onChange={(event) => {
              const next = new URLSearchParams(searchParams);
              const value = event.target.value;
              if (value) next.set('provider', value);
              else next.delete('provider');
              navigate(`/auth-profiles?${next.toString()}`);
            }}
          >
            <option value="">All providers</option>
            {providers.map((provider) => (
              <option key={provider.name} value={provider.name}>{provider.name}</option>
            ))}
          </select>
          <button className="btn btn-primary" onClick={openCreate}>
            <Plus size={16} />
            Add Auth Profile
          </button>
        </div>
      </div>

      {notice && (
        <div className={`alert alert-${notice.type}`} style={{ marginBottom: '1.5rem' }}>
          {notice.message}
        </div>
      )}

      <div className="card">
        <div className="card-header">
          <h3>Credential Inventory</h3>
        </div>
        <div className="card-body">
          {isLoading ? (
            <div className="empty-state"><KeyRound size={48} /><p>Loading auth profiles...</p></div>
          ) : filteredProfiles.length === 0 ? (
            <div className="empty-state">
              <KeyRound size={48} />
              <p>No auth profiles yet. Create one or add a provider first.</p>
            </div>
          ) : (
            <div className="table-container">
              <table className="table">
                <thead>
                  <tr>
                    <th>Profile</th>
                    <th>Mode</th>
                    <th>Status</th>
                    <th>Identity</th>
                    <th>Routing</th>
                    <th>Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {filteredProfiles.map((profile) => (
                    <tr key={profile.qualified_name} data-testid={`auth-profile-row-${profile.qualified_name}`}>
                      <td>
                        <div style={{ fontWeight: 600 }}>{profile.qualified_name}</div>
                        <div className="text-muted" style={{ fontSize: '0.85rem' }}>{profile.format}</div>
                      </td>
                      <td>{modeLabel(profile.mode)}</td>
                      <td>
                        <StatusBadge
                          status={profile.connected ? 'active' : 'inactive'}
                          label={profile.connected ? 'Connected' : 'Disconnected'}
                        />
                      </td>
                      <td>
                        <div>{profile.email || profile.account_id || profile.secret_masked || profile.access_token_masked || 'No runtime identity'}</div>
                        <div className="text-muted" style={{ fontSize: '0.85rem' }}>
                          {profile.expires_at ? `Expires ${new Date(profile.expires_at).toLocaleString()}` : 'No expiry'}
                        </div>
                      </td>
                      <td>
                        <div>weight {profile.weight}</div>
                        <div className="text-muted" style={{ fontSize: '0.85rem' }}>
                          {[profile.region, profile.prefix].filter(Boolean).join(' · ') || 'default'}
                        </div>
                      </td>
                      <td>
                        <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
                          <button className="btn btn-ghost btn-sm" title="Edit" onClick={() => openEdit(profile)}>
                            <Pencil size={14} />
                          </button>
                          {profile.mode === 'codex-oauth' && (
                            <>
                              <button
                                className="btn btn-ghost btn-sm"
                                title={profile.connected ? 'Reconnect OAuth' : 'Connect OAuth'}
                                onClick={() => void handleConnect(profile)}
                                disabled={connecting === profile.qualified_name}
                              >
                                <LinkIcon size={14} />
                              </button>
                              <button
                                className="btn btn-ghost btn-sm"
                                title="Refresh token"
                                onClick={() => void handleRefresh(profile)}
                                disabled={!profile.connected || refreshing === profile.qualified_name}
                              >
                                <RefreshCw size={14} className={refreshing === profile.qualified_name ? 'spinning' : ''} />
                              </button>
                            </>
                          )}
                          {profile.mode === 'anthropic-claude-subscription' && (
                            <button
                              className="btn btn-ghost btn-sm"
                              title={profile.connected ? 'Reconnect token' : 'Connect token'}
                              onClick={() => void handleConnect(profile)}
                              disabled={connecting === profile.qualified_name}
                            >
                              <LinkIcon size={14} />
                            </button>
                          )}
                          <button className="btn btn-ghost btn-sm" title="Delete" onClick={() => void handleDelete(profile)}>
                            <Trash2 size={14} />
                          </button>
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      </div>

      {showModal && (
        <div className="modal-overlay" onClick={closeModal}>
          <div className="modal" onClick={(event) => event.stopPropagation()}>
            <div className="modal-header">
              <h3>{editing ? 'Edit Auth Profile' : 'Add Auth Profile'}</h3>
              <button className="btn btn-ghost btn-sm" onClick={closeModal}>
                <X size={18} />
              </button>
            </div>
            <div className="modal-body">
              {error && <div className="form-error">{error}</div>}

              <div className="form-group">
                <label>Provider</label>
                <select
                  value={form.provider}
                  onChange={(event) => {
                    const provider = event.target.value;
                    setForm((prev) => ({
                      ...prev,
                      provider,
                      mode: normalizeModeForProvider(provider, prev.mode),
                      secret: '',
                    }));
                  }}
                  disabled={!!editing}
                >
                  <option value="">Select provider</option>
                  {providers.map((provider) => (
                    <option key={provider.name} value={provider.name}>{provider.name}</option>
                  ))}
                </select>
              </div>

              <div className="form-group">
                <label>Profile ID</label>
                <input
                  type="text"
                  value={form.id}
                  onChange={(event) => setForm((prev) => ({ ...prev, id: event.target.value }))}
                  placeholder="e.g. billing, oauth-user"
                  disabled={!!editing}
                />
              </div>

              <div className="form-group">
                <label>Mode</label>
                <select
                  value={form.mode}
                  onChange={(event) => setForm((prev) => ({ ...prev, mode: event.target.value as AuthMode, secret: '' }))}
                >
                  {allowedModesForProvider(form.provider).map((mode) => (
                    <option key={mode} value={mode}>{modeLabel(mode)}</option>
                  ))}
                </select>
              </div>

              {!isManagedMode(form.mode) ? (
                <div className="form-group">
                  <label>Secret {editing && '(leave empty to keep current)'}</label>
                  <input
                    type="password"
                    value={form.secret}
                    onChange={(event) => setForm((prev) => ({ ...prev, secret: event.target.value }))}
                    placeholder="sk-..."
                  />
                </div>
              ) : (
                <div className="alert alert-warning" style={{ marginBottom: '1rem' }}>
                  {form.mode === 'codex-oauth'
                    ? 'OAuth tokens are managed separately from config. Save this profile, then use the link action to connect it.'
                    : 'Claude subscription tokens are managed separately from config. Save this profile, then use the link action to paste a `claude setup-token`. This mode is intended for the official Anthropic endpoint only.'}
                </div>
              )}

              <div className="form-group">
                <label>Weight</label>
                <input
                  type="number"
                  min={1}
                  value={form.weight}
                  onChange={(event) => setForm((prev) => ({ ...prev, weight: Number(event.target.value || 1) }))}
                />
              </div>

              <div className="form-group">
                <label>Region</label>
                <input
                  type="text"
                  value={form.region}
                  onChange={(event) => setForm((prev) => ({ ...prev, region: event.target.value }))}
                  placeholder="optional"
                />
              </div>

              <div className="form-group">
                <label>Prefix</label>
                <input
                  type="text"
                  value={form.prefix}
                  onChange={(event) => setForm((prev) => ({ ...prev, prefix: event.target.value }))}
                  placeholder="optional"
                />
              </div>

              <div className="form-group">
                <label style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                  <input
                    type="checkbox"
                    checked={form.disabled}
                    onChange={(event) => setForm((prev) => ({ ...prev, disabled: event.target.checked }))}
                  />
                  Disabled
                </label>
              </div>
            </div>
            <div className="modal-footer">
              <button className="btn btn-secondary" onClick={closeModal}>Cancel</button>
              <button className="btn btn-primary" onClick={() => void handleSave()} disabled={saving}>
                {saving ? 'Saving...' : editing ? 'Update' : 'Create'}
              </button>
            </div>
          </div>
        </div>
      )}

      {connectProfile && (
        <div className="modal-overlay" onClick={closeConnectModal}>
          <div className="modal" onClick={(event) => event.stopPropagation()}>
            <div className="modal-header">
              <h3>Connect Auth Profile</h3>
              <button className="btn btn-ghost btn-sm" onClick={closeConnectModal}>
                <X size={18} />
              </button>
            </div>
            <div className="modal-body">
              <p className="page-subtitle" style={{ marginBottom: '1rem' }}>
                {connectProfile.mode === 'anthropic-claude-subscription'
                  ? <>Paste a Claude setup-token for <strong>{connectProfile.qualified_name}</strong>. The token is stored in the runtime auth sidecar, not in config.</>
                  : <>Connect <strong>{connectProfile.qualified_name}</strong> with a browser OAuth flow, device code flow, or a server-local Codex auth bundle import.</>}
              </p>
              {connectError && <div className="form-error">{connectError}</div>}
              {connectProfile.mode === 'anthropic-claude-subscription' ? (
                <div className="form-group">
                  <label>Setup Token</label>
                  <input
                    type="password"
                    value={connectSecret}
                    onChange={(event) => setConnectSecret(event.target.value)}
                    placeholder="sk-ant-oat01-..."
                  />
                </div>
              ) : (
                <div style={{ display: 'grid', gap: 16 }}>
                  <div className="alert alert-warning">
                    Codex managed auth is pinned to official OpenAI hosts. Browser OAuth completes in your browser, but token exchange, device flow, and refresh run from Prism server egress <code>{authEgressLabel}</code>. If that egress is blocked or in an unsupported region, auth will fail even with a valid auth.json.
                  </div>
                  <div className="card" style={{ marginTop: 4 }}>
                    <div className="card-body" style={{ display: 'grid', gap: 8 }}>
                      <div><strong>Runtime auth dir:</strong> <code>{managedAuthRuntime?.storage_dir ?? 'unavailable'}</code></div>
                      <div><strong>Default import file:</strong> <code>{defaultCodexAuthFile}</code></div>
                      <div><strong>Auth egress:</strong> <code>{authEgressLabel}</code></div>
                    </div>
                  </div>
                  <div className="form-group" style={{ marginBottom: 0 }}>
                    <label>Server-local auth.json path</label>
                    <input
                      type="text"
                      value={connectImportPath}
                      onChange={(event) => setConnectImportPath(event.target.value)}
                      placeholder={defaultCodexAuthFile}
                    />
                    <div className="text-muted" style={{ fontSize: '0.85rem' }}>
                      Leave this as-is to import from the configured default path, or replace it with any server-local file Prism can read.
                    </div>
                  </div>
                  <div style={{ display: 'grid', gap: 12 }}>
                    <button
                      className="btn btn-secondary"
                      onClick={() => void handleImportLocalCodex()}
                      disabled={connecting === connectProfile.qualified_name}
                    >
                      <Laptop size={16} />
                      Import server-local auth.json
                    </button>
                    <button
                      className="btn btn-secondary"
                      onClick={() => void handleStartDeviceFlow()}
                      disabled={connecting === connectProfile.qualified_name}
                    >
                      <Smartphone size={16} />
                      Start Device Flow
                    </button>
                    <button
                      className="btn btn-secondary"
                      onClick={() => void handleStartBrowserOauth()}
                      disabled={connecting === connectProfile.qualified_name}
                    >
                      <Globe size={16} />
                      Open Browser OAuth
                    </button>
                  </div>
                  {codexDevice && (
                    <div className="card" style={{ marginTop: 4 }}>
                      <div className="card-body" style={{ display: 'grid', gap: 8 }}>
                        <div><strong>User Code:</strong> {codexDevice.user_code}</div>
                        <div>
                          <strong>Verification URL:</strong>{' '}
                          <a href={codexDevice.verification_url} target="_blank" rel="noreferrer">
                            {codexDevice.verification_url}
                          </a>
                        </div>
                        <div className="text-muted" style={{ fontSize: '0.9rem' }}>
                          Polling every {codexDevice.interval_secs}s until the device flow completes.
                        </div>
                      </div>
                    </div>
                  )}
                </div>
              )}
            </div>
            <div className="modal-footer">
              <button className="btn btn-secondary" onClick={closeConnectModal}>Cancel</button>
              {connectProfile.mode === 'anthropic-claude-subscription' && (
                <button
                  className="btn btn-primary"
                  onClick={() => void handleConnectToken()}
                  disabled={connecting === connectProfile.qualified_name}
                >
                  {connecting === connectProfile.qualified_name ? 'Connecting...' : 'Connect'}
                </button>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
