import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { providersApi } from '../services/api';
import type { Provider, ProviderAuthProfile, ProviderCreateRequest, FormatType, UpstreamType, ProfileKind, ActivationMode, PresentationPreviewResponse, ProviderHealthResult } from '../types';
import StatusBadge from '../components/StatusBadge';
import TagList from '../components/TagList';
import { Server, Plus, Pencil, Trash2, X, RefreshCw, HeartPulse, PlusCircle, MinusCircle, Copy, Eye, ChevronDown, ChevronUp } from 'lucide-react';

const FORMAT_OPTIONS: { value: FormatType; label: string }[] = [
  { value: 'openai', label: 'OpenAI' },
  { value: 'claude', label: 'Claude (Anthropic)' },
  { value: 'gemini', label: 'Gemini (Google)' },
];

const DEFAULT_BASE_URLS: Record<UpstreamType, string> = {
  openai: 'https://api.openai.com',
  codex: 'https://chatgpt.com/backend-api/codex',
  claude: 'https://api.anthropic.com',
  gemini: 'https://generativelanguage.googleapis.com',
};

const UPSTREAM_OPTIONS: Record<FormatType, Array<{ value: UpstreamType; label: string; description: string }>> = {
  openai: [
    { value: 'openai', label: 'OpenAI API', description: 'Usage-billed OpenAI-compatible APIs such as OpenAI, DeepSeek, Groq' },
    { value: 'codex', label: 'Codex', description: 'ChatGPT/Codex subscription backend at chatgpt.com/backend-api/codex' },
  ],
  claude: [
    { value: 'claude', label: 'Claude', description: 'Anthropic Claude API' },
  ],
  gemini: [
    { value: 'gemini', label: 'Gemini', description: 'Google Gemini API' },
  ],
};

const PROFILE_OPTIONS: { value: ProfileKind; label: string; description: string }[] = [
  { value: 'native', label: 'Native', description: 'No identity headers or body mutations' },
  { value: 'claude-code', label: 'Claude Code', description: 'Claude Code client identity (headers + body mutations)' },
  { value: 'gemini-cli', label: 'Gemini CLI', description: 'Gemini CLI client identity (headers only)' },
  { value: 'codex-cli', label: 'Codex CLI', description: 'Codex CLI client identity (headers only)' },
];

interface HeaderPair {
  key: string;
  value: string;
}

interface FormState {
  name: string;
  format: FormatType;
  upstream: UpstreamType;
  base_url: string;
  proxy_url: string;
  api_key: string;
  prefix: string;
  disabled: boolean;
  models: string;
  excluded_models: string;
  headers: HeaderPair[];
  wire_api: string;
  weight: number;
  region: string;
  // Upstream Presentation
  profile: ProfileKind;
  activation_mode: ActivationMode;
  strict_mode: boolean;
  sensitive_words: string;
  cache_user_id: boolean;
  presentation_headers: HeaderPair[];
}

interface NoticeState {
  type: 'success' | 'error' | 'warning';
  message: string;
}

const emptyForm: FormState = {
  name: '',
  format: 'openai',
  upstream: 'openai',
  base_url: DEFAULT_BASE_URLS.openai,
  proxy_url: '',
  api_key: '',
  prefix: '',
  disabled: false,
  models: '',
  excluded_models: '',
  headers: [],
  wire_api: 'chat',
  weight: 1,
  region: '',
  profile: 'native',
  activation_mode: 'always',
  strict_mode: false,
  sensitive_words: '',
  cache_user_id: false,
  presentation_headers: [],
};

export default function Providers() {
  const navigate = useNavigate();
  const [providers, setProviders] = useState<Provider[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [showModal, setShowModal] = useState(false);
  const [editName, setEditName] = useState<string | null>(null);
  const [form, setForm] = useState<FormState>(emptyForm);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState<NoticeState | null>(null);
  const [saving, setSaving] = useState(false);
  const [fetchingModels, setFetchingModels] = useState(false);
  const [healthChecking, setHealthChecking] = useState<string | null>(null);
  const [healthResults, setHealthResults] = useState<Record<string, ProviderHealthResult>>({});
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [previewResult, setPreviewResult] = useState<PresentationPreviewResponse | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);
  const [currentAuthProfiles, setCurrentAuthProfiles] = useState<ProviderAuthProfile[]>([]);

  const formatAuthProfileMode = (mode: ProviderAuthProfile['mode']) => {
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
  };

  const extractErrorMessage = (err: unknown, fallback: string) => {
    if (typeof err === 'object' && err !== null) {
      const maybeError = err as {
        message?: unknown;
        response?: { data?: { message?: unknown } };
      };
      const apiMessage = maybeError.response?.data?.message;
      if (typeof apiMessage === 'string' && apiMessage.trim()) {
        return apiMessage;
      }
      if (typeof maybeError.message === 'string' && maybeError.message.trim()) {
        return maybeError.message;
      }
    }
    return fallback;
  };

  const fetchProviders = async (options?: { surfaceError?: boolean }) => {
    const surfaceError = options?.surfaceError ?? true;
    try {
      const response = await providersApi.list();
      setProviders(response.data);
      return response.data;
    } catch (err) {
      console.error('Failed to fetch providers:', err);
      if (surfaceError) {
        setNotice({
          type: 'error',
          message: extractErrorMessage(err, 'Failed to fetch providers'),
        });
      }
      throw err;
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    void fetchProviders();
  }, []);

  const openCreate = () => {
    setEditName(null);
    setForm(emptyForm);
    setCurrentAuthProfiles([]);
    setError('');
    setShowAdvanced(false);
    setPreviewResult(null);
    setShowModal(true);
  };

  const openEdit = async (provider: Provider) => {
    setEditName(provider.name);
    setError('');
    setShowAdvanced(false);
    setPreviewResult(null);

    // Fetch full provider details (includes upstream_presentation)
    let detail = provider;
    try {
      const res = await providersApi.get(provider.name);
      detail = res.data;
    } catch {
      // Fall back to list data
    }

    const headerPairs: HeaderPair[] = detail.headers
      ? Object.entries(detail.headers).map(([key, value]) => ({ key, value }))
      : [];
    const modelStrings = (detail.models || []).map((m) =>
      typeof m === 'string' ? m : m.id
    );

    const pres = detail.upstream_presentation;
    const presHeaders: HeaderPair[] = pres?.['custom-headers']
      ? Object.entries(pres['custom-headers']).map(([key, value]) => ({ key, value }))
      : [];
    setCurrentAuthProfiles(detail.auth_profiles ?? provider.auth_profiles ?? []);

    setForm({
      name: detail.name,
      format: detail.format,
      upstream: detail.upstream,
      base_url: detail.base_url ?? '',
      proxy_url: detail.proxy_url ?? '',
      api_key: '',
      prefix: detail.prefix ?? '',
      disabled: detail.disabled,
      models: modelStrings.join(', '),
      excluded_models: (detail.excluded_models || []).join(', '),
      headers: headerPairs,
      wire_api: detail.wire_api ?? 'chat',
      weight: detail.weight ?? 1,
      region: detail.region ?? '',
      profile: pres?.profile ?? 'native',
      activation_mode: pres?.mode ?? 'always',
      strict_mode: pres?.['strict-mode'] ?? false,
      sensitive_words: (pres?.['sensitive-words'] ?? []).join(', '),
      cache_user_id: pres?.['cache-user-id'] ?? false,
      presentation_headers: presHeaders,
    });
    setShowModal(true);
  };

  const handleSubmit = async () => {
    const providerName = form.name.trim();
    if (!providerName) {
      setError('Name is required');
      return;
    }
    setSaving(true);
    setError('');

    try {
      const models = form.models
        .split(',')
        .map((m) => m.trim())
        .filter(Boolean);

      const excluded_models = form.excluded_models
        .split(',')
        .map((m) => m.trim())
        .filter(Boolean);

      const headers: Record<string, string> = {};
      form.headers.forEach(({ key, value }) => {
        if (key.trim() && value.trim()) headers[key.trim()] = value.trim();
      });

      // Build upstream_presentation
      const presCustomHeaders: Record<string, string> = {};
      form.presentation_headers.forEach(({ key, value }) => {
        if (key.trim() && value.trim()) presCustomHeaders[key.trim()] = value.trim();
      });
      const sensitiveWords = form.sensitive_words
        .split(',')
        .map((w) => w.trim())
        .filter(Boolean);
      const upstream_presentation = {
        profile: form.profile,
        mode: form.activation_mode,
        'strict-mode': form.strict_mode,
        'sensitive-words': sensitiveWords,
        'cache-user-id': form.cache_user_id,
        'custom-headers': presCustomHeaders,
      };

      if (editName) {
        await providersApi.update(editName, {
          base_url: form.base_url || null,
          proxy_url: form.proxy_url || null,
          upstream: form.upstream,
          api_key: form.upstream === 'codex' ? undefined : (form.api_key || undefined),
          prefix: form.prefix || null,
          disabled: form.disabled,
          models,
          excluded_models,
          headers,
          wire_api: form.wire_api,
          weight: form.weight,
          region: form.region || null,
          upstream_presentation,
        });
      } else {
        const data: ProviderCreateRequest = {
          name: providerName,
          format: form.format,
          upstream: form.upstream,
          base_url: form.base_url || undefined,
          proxy_url: form.proxy_url || undefined,
          api_key: form.upstream === 'codex' ? undefined : form.api_key,
          prefix: form.prefix || undefined,
          disabled: form.disabled,
          models,
          excluded_models,
          headers,
          wire_api: form.wire_api,
          weight: form.weight,
          region: form.region || undefined,
          upstream_presentation,
        };
        await providersApi.create(data);
      }

      const persistedProviderName = editName ?? providerName;
      setShowModal(false);
      setForm(emptyForm);

      try {
        const refreshedProviders = await fetchProviders({ surfaceError: false });
        const providerExists = refreshedProviders.some((provider) => provider.name === persistedProviderName);
        setNotice(
          providerExists
            ? {
                type: 'success',
                message: `Provider "${persistedProviderName}" ${editName ? 'updated' : 'created'} successfully.`,
              }
            : {
                type: 'warning',
                message: `Provider "${persistedProviderName}" ${editName ? 'updated' : 'created'} successfully, but the refreshed list did not include it.`,
              }
        );
      } catch (refreshErr) {
        setNotice({
          type: 'warning',
          message: `Provider "${persistedProviderName}" ${editName ? 'updated' : 'created'} successfully, but refreshing the list failed: ${extractErrorMessage(refreshErr, 'Failed to fetch providers')}`,
        });
      }
    } catch (err) {
      setError(extractErrorMessage(err, 'Failed to save provider'));
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async (name: string) => {
    if (!window.confirm(`Delete provider "${name}"? This cannot be undone.`)) {
      return;
    }

    try {
      await providersApi.delete(name);
      try {
        await fetchProviders({ surfaceError: false });
        setNotice({ type: 'success', message: `Provider "${name}" deleted successfully.` });
      } catch (refreshErr) {
        setNotice({
          type: 'warning',
          message: `Provider "${name}" deleted successfully, but refreshing the list failed: ${extractErrorMessage(refreshErr, 'Failed to fetch providers')}`,
        });
      }
    } catch (err) {
      console.error('Failed to delete provider:', err);
      setNotice({
        type: 'error',
        message: extractErrorMessage(err, `Failed to delete provider "${name}"`),
      });
    }
  };

  const handleFormatChange = (fmt: FormatType) => {
    const nextUpstream = UPSTREAM_OPTIONS[fmt][0]?.value ?? fmt;
    setForm((prev) => ({
      ...prev,
      format: fmt,
      upstream: nextUpstream,
      wire_api: nextUpstream === 'codex' ? 'responses' : prev.wire_api,
      api_key: nextUpstream === 'codex' ? '' : prev.api_key,
      base_url: prev.base_url === DEFAULT_BASE_URLS[prev.upstream]
        ? DEFAULT_BASE_URLS[nextUpstream]
        : prev.base_url,
    }));
  };

  const handleUpstreamChange = (upstream: UpstreamType) => {
    setForm((prev) => ({
      ...prev,
      upstream,
      wire_api: upstream === 'codex' ? 'responses' : (prev.wire_api === 'responses' ? prev.wire_api : 'chat'),
      api_key: upstream === 'codex' ? '' : prev.api_key,
      base_url: prev.base_url === DEFAULT_BASE_URLS[prev.upstream]
        ? DEFAULT_BASE_URLS[upstream]
        : prev.base_url,
    }));
  };

  const handleFetchModels = async () => {
    setFetchingModels(true);
    try {
      const result = await providersApi.fetchModels({
        format: form.format,
        upstream: form.upstream,
        api_key: form.api_key,
        base_url: form.base_url || undefined,
      });
      setForm((prev) => ({ ...prev, models: result.join(', ') }));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to fetch models');
    } finally {
      setFetchingModels(false);
    }
  };

  const handleHealthCheck = async (name: string) => {
    setHealthChecking(name);
    try {
      const result = await providersApi.healthCheck(name);
      setHealthResults((prev) => ({ ...prev, [name]: result }));
    } catch (err) {
      setHealthResults((prev) => ({
        ...prev,
        [name]: {
          provider: name,
          upstream: 'unknown',
          status: 'error',
          checked_at: new Date().toISOString(),
          latency_ms: 0,
          checks: [{
            capability: 'probe',
            status: 'failed',
            message: err instanceof Error ? err.message : 'Health check failed',
          }],
        },
      }));
    } finally {
      setHealthChecking(null);
    }
  };

  const summarizeProbe = (result: ProviderHealthResult) =>
    result.checks
      .filter((check) => check.capability !== 'auth')
      .map((check) => `${check.capability}:${check.status}`)
      .join(' · ');

  const handlePresentationPreview = async () => {
    if (!editName) return;
    setPreviewLoading(true);
    try {
      const result = await providersApi.presentationPreview(editName, {
        model: form.models.split(',')[0]?.trim() || undefined,
      });
      setPreviewResult(result);
    } catch (err) {
      setError(extractErrorMessage(err, 'Failed to generate presentation preview'));
    } finally {
      setPreviewLoading(false);
    }
  };

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h2>Providers</h2>
          <p className="page-subtitle">Manage AI provider connections</p>
        </div>
        <button className="btn btn-primary" onClick={openCreate}>
          <Plus size={16} />
          Add Provider
        </button>
      </div>

      {notice && (
        <div className={`alert alert-${notice.type}`} style={{ marginBottom: '1.5rem' }}>
          {notice.message}
        </div>
      )}

      <div className="card">
        <div className="table-wrapper">
          <table className="table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Identity</th>
                <th>Presentation</th>
                <th>Wire API</th>
                <th>Base URL</th>
                <th>Models</th>
                <th>Status</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              {isLoading ? (
                <tr>
                  <td colSpan={8} className="table-empty">Loading...</td>
                </tr>
              ) : providers.length === 0 ? (
                <tr>
                  <td colSpan={8} className="table-empty">
                    <div className="empty-state">
                      <Server size={48} />
                      <p>No providers configured</p>
                      <button className="btn btn-primary" onClick={openCreate}>
                        <Plus size={16} />
                        Add First Provider
                      </button>
                    </div>
                  </td>
                </tr>
              ) : (
                providers.map((provider) => (
                  <tr key={provider.name}>
                    <td>
                      <div className="text-bold">{provider.name}</div>
                      {provider.auth_profiles && provider.auth_profiles.length > 0 && (
                        <div
                          className="text-muted"
                          data-testid={`provider-auth-profiles-${provider.name}`}
                          style={{ fontSize: '0.8rem', marginTop: 4 }}
                        >
                          Auth profiles: {provider.auth_profiles.map((profile) => profile.id).join(', ')}
                        </div>
                      )}
                    </td>
                    <td>
                      <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
                        <span className="type-badge">
                          {FORMAT_OPTIONS.find((t) => t.value === provider.format)?.label ?? provider.format}
                        </span>
                        <span className="type-badge" style={{ opacity: 0.85 }}>
                          {UPSTREAM_OPTIONS[provider.format].find((t) => t.value === provider.upstream)?.label ?? provider.upstream}
                        </span>
                      </div>
                    </td>
                    <td>
                      <span className="type-badge" style={{ opacity: provider.upstream_presentation?.profile && provider.upstream_presentation.profile !== 'native' ? 1 : 0.5 }}>
                        {PROFILE_OPTIONS.find((p) => p.value === provider.upstream_presentation?.profile)?.label ?? 'Native'}
                      </span>
                    </td>
                    <td>
                      <span className="type-badge">{provider.wire_api}</span>
                    </td>
                    <td className="text-mono" style={{ maxWidth: 250 }}>
                      <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                        <span className="text-ellipsis" style={{ flex: 1 }} title={provider.base_url ?? undefined}>
                          {provider.base_url}
                        </span>
                        {provider.base_url && (
                          <button
                            className="btn btn-ghost btn-sm"
                            onClick={() => {
                              navigator.clipboard.writeText(provider.base_url ?? '');
                            }}
                            title="Copy URL"
                            style={{ flexShrink: 0, padding: '2px 4px' }}
                          >
                            <Copy size={12} />
                          </button>
                        )}
                      </div>
                    </td>
                    <td>
                      {provider.models.length > 0 ? (
                        <TagList items={provider.models.map((m) => m.id)} maxVisible={3} />
                      ) : (
                        <span className="text-muted">-</span>
                      )}
                    </td>
                    <td>
                      {healthResults[provider.name] ? (
                        <div
                          className={`health-badge ${
                            healthResults[provider.name].status === 'ok'
                              ? 'health-ok'
                              : healthResults[provider.name].status === 'warning'
                                ? 'health-warning'
                                : 'health-error'
                          }`}
                          title={healthResults[provider.name].checks
                            .map((check) => `${check.capability}: ${check.status}${check.message ? ` (${check.message})` : ''}`)
                            .join('\n')}
                        >
                          {healthResults[provider.name].status === 'ok'
                            ? `✓ ${healthResults[provider.name].latency_ms}ms`
                            : healthResults[provider.name].status === 'warning'
                              ? `! ${healthResults[provider.name].latency_ms}ms`
                              : '✗ Error'}
                          <div className="text-muted" style={{ fontSize: '0.72rem', marginTop: 2 }}>
                            {summarizeProbe(healthResults[provider.name])}
                          </div>
                        </div>
                      ) : (
                        <StatusBadge
                          status={provider.disabled ? 'inactive' : 'active'}
                        />
                      )}
                    </td>
                    <td>
                      <div className="action-btns">
                        <button
                          className="btn btn-ghost btn-sm"
                          onClick={() => openEdit(provider)}
                          title="Edit"
                        >
                          <Pencil size={14} />
                        </button>
                        <button
                          className="btn btn-ghost btn-sm"
                          onClick={() => handleHealthCheck(provider.name)}
                          title="Health Check"
                          disabled={healthChecking === provider.name}
                        >
                          <HeartPulse size={14} />
                        </button>
                        <button
                          className="btn btn-ghost btn-sm btn-danger-ghost"
                          onClick={() => handleDelete(provider.name)}
                          title="Delete"
                        >
                          <Trash2 size={14} />
                        </button>
                      </div>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* Modal */}
      {showModal && (
        <div className="modal-overlay" onClick={() => setShowModal(false)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3>{editName ? 'Edit Provider' : 'Add Provider'}</h3>
              <button className="btn btn-ghost btn-sm" onClick={() => setShowModal(false)}>
                <X size={18} />
              </button>
            </div>
            <div className="modal-body">
              {error && <div className="form-error">{error}</div>}
              {editName && currentAuthProfiles.length > 0 && (
                <div className="alert alert-warning" style={{ marginBottom: '1rem' }}>
                  <div style={{ fontWeight: 600, marginBottom: 6 }}>
                    This provider uses managed auth profiles
                  </div>
                  <div style={{ fontSize: '0.9rem', opacity: 0.9, marginBottom: 8 }}>
                    Shared provider fields can be edited here. Credential material is managed from the dedicated Auth Profiles page.
                  </div>
                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    onClick={() => navigate(`/auth-profiles?provider=${encodeURIComponent(editName)}`)}
                  >
                    Manage Auth Profiles
                  </button>
                  <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                    {currentAuthProfiles.map((profile) => (
                      <span
                        key={profile.qualified_name}
                        className="type-badge"
                        title={`${profile.qualified_name} · ${formatAuthProfileMode(profile.mode)}`}
                      >
                        {profile.id} · {formatAuthProfileMode(profile.mode)}
                      </span>
                    ))}
                  </div>
                </div>
              )}

              <div className="form-group">
                <label>Name</label>
                <input
                  type="text"
                  value={form.name}
                  onChange={(e) => setForm({ ...form, name: e.target.value })}
                  placeholder="e.g., deepseek, openai-prod"
                  disabled={!!editName}
                />
                <span className="form-help" style={{ fontSize: '0.8rem', opacity: 0.6 }}>
                  Unique identifier for this provider. Used in routing and logs.
                </span>
              </div>

              <div className="form-group">
                <label>Format</label>
                <select
                  value={form.format}
                  onChange={(e) => handleFormatChange(e.target.value as FormatType)}
                  disabled={!!editName}
                >
                  {FORMAT_OPTIONS.map((t) => (
                    <option key={t.value} value={t.value}>{t.label}</option>
                  ))}
                </select>
                <span className="form-help" style={{ fontSize: '0.8rem', opacity: 0.6 }}>
                  Wire protocol format. Use OpenAI for OpenAI-compatible providers (DeepSeek, Groq, etc.).
                </span>
              </div>

              <div className="form-group">
                <label>Upstream Family</label>
                <select
                  value={form.upstream}
                  onChange={(e) => handleUpstreamChange(e.target.value as UpstreamType)}
                >
                  {UPSTREAM_OPTIONS[form.format].map((option) => (
                    <option key={option.value} value={option.value}>{option.label}</option>
                  ))}
                </select>
                <span className="form-help" style={{ fontSize: '0.8rem', opacity: 0.6 }}>
                  {UPSTREAM_OPTIONS[form.format].find((option) => option.value === form.upstream)?.description}
                </span>
              </div>

              <div className="form-group">
                <label>Base URL</label>
                <input
                  type="text"
                  value={form.base_url}
                  onChange={(e) => setForm({ ...form, base_url: e.target.value })}
                  placeholder={DEFAULT_BASE_URLS[form.upstream]}
                />
              </div>

              {form.upstream !== 'codex' ? (
                <div className="form-group">
                  <label>API Key {editName && '(leave empty to keep current)'}</label>
                  <input
                    type="password"
                    value={form.api_key}
                    onChange={(e) => setForm({ ...form, api_key: e.target.value })}
                    placeholder={editName ? '********' : 'sk-...'}
                  />
                  <span className="form-help" style={{ fontSize: '0.8rem', opacity: 0.6 }}>
                    Optional when this provider will be backed by explicit auth profiles.
                  </span>
                </div>
              ) : (
                <div className="alert alert-warning" style={{ marginBottom: '1rem' }}>
                  Codex providers do not accept static API keys. Create a `Codex OAuth` auth profile after saving this provider.
                </div>
              )}

              <div className="form-group">
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                  <label>Models (comma-separated)</label>
                  <button
                    type="button"
                    className="btn btn-ghost btn-sm"
                    onClick={handleFetchModels}
                    disabled={form.upstream === 'codex' || fetchingModels || (!form.api_key && !editName)}
                    title={form.upstream === 'codex'
                      ? 'Codex model discovery is not supported'
                      : (editName && !form.api_key ? 'Enter API key to fetch models' : 'Fetch available models from provider')}
                  >
                    <RefreshCw size={14} className={fetchingModels ? 'spinning' : ''} />
                    {fetchingModels ? 'Fetching...' : 'Fetch Models'}
                  </button>
                </div>
                <input
                  type="text"
                  value={form.models}
                  onChange={(e) => setForm({ ...form, models: e.target.value })}
                  placeholder="gpt-4o, gpt-4o-mini, gpt-3.5-turbo"
                />
              </div>

              {/* ── Upstream Presentation ── */}
              <fieldset style={{ border: '1px solid var(--color-border)', borderRadius: 8, padding: '12px 16px', marginBottom: 0 }}>
                <legend style={{ fontSize: '0.85rem', fontWeight: 600, padding: '0 6px' }}>Upstream Presentation</legend>

                <div className="form-group" style={{ marginBottom: 12 }}>
                  <label>Profile</label>
                  <select
                    value={form.profile}
                    onChange={(e) => setForm({ ...form, profile: e.target.value as ProfileKind })}
                  >
                    {PROFILE_OPTIONS.map((p) => (
                      <option key={p.value} value={p.value}>{p.label}</option>
                    ))}
                  </select>
                  <span className="form-help" style={{ fontSize: '0.8rem', opacity: 0.6 }}>
                    {PROFILE_OPTIONS.find((p) => p.value === form.profile)?.description}
                  </span>
                </div>

                {form.profile !== 'native' && (
                  <div className="form-group" style={{ marginBottom: 12 }}>
                    <label>Activation Mode</label>
                    <select
                      value={form.activation_mode}
                      onChange={(e) => setForm({ ...form, activation_mode: e.target.value as ActivationMode })}
                    >
                      <option value="always">Always</option>
                      <option value="auto">Auto (skip if real client detected)</option>
                    </select>
                  </div>
                )}

                {form.profile === 'claude-code' && (
                  <div style={{ background: 'var(--color-bg-secondary)', borderRadius: 6, padding: '10px 12px', marginBottom: 12 }}>
                    <p style={{ fontSize: '0.8rem', fontWeight: 600, margin: '0 0 8px' }}>Claude Code Options</p>

                    <div className="form-group form-group-inline" style={{ marginBottom: 8 }}>
                      <label className="checkbox-label">
                        <input
                          type="checkbox"
                          checked={form.strict_mode}
                          onChange={(e) => setForm({ ...form, strict_mode: e.target.checked })}
                        />
                        Strict Mode
                      </label>
                      <span className="form-help" style={{ fontSize: '0.75rem', opacity: 0.6, marginLeft: 4 }}>
                        Replace user's system prompt instead of prepending
                      </span>
                    </div>

                    <div className="form-group form-group-inline" style={{ marginBottom: 8 }}>
                      <label className="checkbox-label">
                        <input
                          type="checkbox"
                          checked={form.cache_user_id}
                          onChange={(e) => setForm({ ...form, cache_user_id: e.target.checked })}
                        />
                        Cache User ID
                      </label>
                      <span className="form-help" style={{ fontSize: '0.75rem', opacity: 0.6, marginLeft: 4 }}>
                        Deterministic user_id per API key
                      </span>
                    </div>

                    <div className="form-group" style={{ marginBottom: 0 }}>
                      <label>Sensitive Words (comma-separated)</label>
                      <input
                        type="text"
                        value={form.sensitive_words}
                        onChange={(e) => setForm({ ...form, sensitive_words: e.target.value })}
                        placeholder="proxy, prism"
                      />
                      <span className="form-help" style={{ fontSize: '0.75rem', opacity: 0.6 }}>
                        Words to obfuscate with zero-width spaces in requests
                      </span>
                    </div>
                  </div>
                )}

                {/* Custom Headers (presentation-level) */}
                <div className="form-group" style={{ marginBottom: 8 }}>
                  <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                    <label>Custom Headers</label>
                    <button
                      type="button"
                      className="btn btn-ghost btn-sm"
                      onClick={() => setForm({ ...form, presentation_headers: [...form.presentation_headers, { key: '', value: '' }] })}
                    >
                      <PlusCircle size={14} />
                      Add
                    </button>
                  </div>
                  {form.presentation_headers.map((header, idx) => (
                    <div key={idx} className="header-pair">
                      <input
                        type="text"
                        value={header.key}
                        onChange={(e) => {
                          const next = [...form.presentation_headers];
                          next[idx] = { ...next[idx], key: e.target.value };
                          setForm({ ...form, presentation_headers: next });
                        }}
                        placeholder="Header name"
                        className="header-key"
                      />
                      <input
                        type="text"
                        value={header.value}
                        onChange={(e) => {
                          const next = [...form.presentation_headers];
                          next[idx] = { ...next[idx], value: e.target.value };
                          setForm({ ...form, presentation_headers: next });
                        }}
                        placeholder="Header value"
                        className="header-value"
                      />
                      <button
                        type="button"
                        className="btn btn-ghost btn-sm btn-danger-ghost"
                        onClick={() => {
                          const next = form.presentation_headers.filter((_, i) => i !== idx);
                          setForm({ ...form, presentation_headers: next });
                        }}
                      >
                        <MinusCircle size={14} />
                      </button>
                    </div>
                  ))}
                  {form.presentation_headers.length === 0 && (
                    <p className="form-hint" style={{ margin: '4px 0 0', fontSize: '0.75rem', opacity: 0.6 }}>
                      Additional headers sent to upstream. Profile headers are applied automatically.
                    </p>
                  )}
                </div>

                {/* Preview Button (only for existing providers) */}
                {editName && (
                  <div style={{ marginTop: 8 }}>
                    <button
                      type="button"
                      className="btn btn-ghost btn-sm"
                      onClick={handlePresentationPreview}
                      disabled={previewLoading}
                      style={{ width: '100%' }}
                    >
                      <Eye size={14} />
                      {previewLoading ? 'Loading...' : 'Preview Presentation'}
                    </button>

                    {previewResult && (
                      <div style={{ marginTop: 8, background: 'var(--color-bg-secondary)', borderRadius: 6, padding: '10px 12px', fontSize: '0.8rem' }}>
                        <p style={{ fontWeight: 600, margin: '0 0 6px' }}>
                          Profile: {previewResult.profile} — {previewResult.activated ? 'Active' : 'Skipped'}
                        </p>
                        {Object.keys(previewResult.effective_headers).length > 0 && (
                          <div style={{ marginBottom: 6 }}>
                            <p style={{ fontWeight: 500, margin: '0 0 2px' }}>Effective Headers:</p>
                            {Object.entries(previewResult.effective_headers).map(([k, v]) => (
                              <div key={k} className="text-mono" style={{ fontSize: '0.75rem', opacity: 0.8 }}>
                                {k}: {v}
                              </div>
                            ))}
                          </div>
                        )}
                        {previewResult.body_mutations.length > 0 && (
                          <div style={{ marginBottom: 6 }}>
                            <p style={{ fontWeight: 500, margin: '0 0 2px' }}>Body Mutations:</p>
                            {previewResult.body_mutations.map((m, i) => (
                              <div key={i} style={{ fontSize: '0.75rem', opacity: 0.8 }}>
                                {m.kind}: {m.applied ? 'applied' : `skipped${m.reason ? ` (${m.reason})` : ''}`}
                              </div>
                            ))}
                          </div>
                        )}
                        {previewResult.protected_headers_blocked.length > 0 && (
                          <div>
                            <p style={{ fontWeight: 500, margin: '0 0 2px', color: 'var(--color-warning, #b86e00)' }}>Protected (blocked):</p>
                            <span style={{ fontSize: '0.75rem', opacity: 0.8 }}>
                              {previewResult.protected_headers_blocked.join(', ')}
                            </span>
                          </div>
                        )}
                      </div>
                    )}
                  </div>
                )}
              </fieldset>

              {/* ── Advanced Section (collapsible) ── */}
              <div style={{ marginTop: 16 }}>
                <button
                  type="button"
                  className="btn btn-ghost btn-sm"
                  onClick={() => setShowAdvanced(!showAdvanced)}
                  style={{ width: '100%', justifyContent: 'space-between', display: 'flex' }}
                >
                  <span>Advanced Options</span>
                  {showAdvanced ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
                </button>
              </div>

              {showAdvanced && (
                <>
                  <div className="form-group">
                    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                      <label>Legacy Headers</label>
                      <button
                        type="button"
                        className="btn btn-ghost btn-sm"
                        onClick={() => setForm({ ...form, headers: [...form.headers, { key: '', value: '' }] })}
                      >
                        <PlusCircle size={14} />
                        Add Header
                      </button>
                    </div>
                    {form.headers.map((header, idx) => (
                      <div key={idx} className="header-pair">
                        <input
                          type="text"
                          value={header.key}
                          onChange={(e) => {
                            const next = [...form.headers];
                            next[idx] = { ...next[idx], key: e.target.value };
                            setForm({ ...form, headers: next });
                          }}
                          placeholder="Header name"
                          className="header-key"
                        />
                        <input
                          type="text"
                          value={header.value}
                          onChange={(e) => {
                            const next = [...form.headers];
                            next[idx] = { ...next[idx], value: e.target.value };
                            setForm({ ...form, headers: next });
                          }}
                          placeholder="Header value"
                          className="header-value"
                        />
                        <button
                          type="button"
                          className="btn btn-ghost btn-sm btn-danger-ghost"
                          onClick={() => {
                            const next = form.headers.filter((_, i) => i !== idx);
                            setForm({ ...form, headers: next });
                          }}
                        >
                          <MinusCircle size={14} />
                        </button>
                      </div>
                    ))}
                    {form.headers.length === 0 && (
                      <p className="form-hint" style={{ margin: '4px 0 0', fontSize: '0.8rem', opacity: 0.6 }}>
                        Legacy headers field. Use "Custom Headers" in Upstream Presentation instead.
                      </p>
                    )}
                  </div>
                </>
              )}

              <div className="form-group">
                <label>Excluded Models (comma-separated)</label>
                <input
                  type="text"
                  value={form.excluded_models}
                  onChange={(e) => setForm({ ...form, excluded_models: e.target.value })}
                  placeholder="gpt-4-vision-*, dall-e-*"
                />
                <span className="form-help" style={{ fontSize: '0.8rem', opacity: 0.6 }}>
                  Models matching these patterns will be excluded from routing.
                </span>
              </div>

              <div className="form-group">
                <label>Prefix</label>
                <input
                  type="text"
                  value={form.prefix}
                  onChange={(e) => setForm({ ...form, prefix: e.target.value })}
                  placeholder="e.g., my-prefix/"
                />
                <span className="form-help" style={{ fontSize: '0.8rem', opacity: 0.6 }}>
                  Optional prefix added to model names when routing.
                </span>
              </div>

              <div className="form-group">
                <label>Proxy URL</label>
                <input
                  type="text"
                  value={form.proxy_url}
                  onChange={(e) => setForm({ ...form, proxy_url: e.target.value })}
                  placeholder="http://proxy:8080 or socks5://proxy:1080"
                />
              </div>

              <div className="form-row">
                {form.format === 'openai' && (
                  <div className="form-group">
                    <label>Wire API</label>
                    <select
                      value={form.wire_api}
                      onChange={(e) => setForm({ ...form, wire_api: e.target.value })}
                      disabled={form.upstream === 'codex'}
                    >
                      <option value="chat">Chat Completions</option>
                      <option value="responses">Responses API</option>
                    </select>
                    {form.upstream === 'codex' && (
                      <span className="form-help" style={{ fontSize: '0.8rem', opacity: 0.6 }}>
                        Codex always uses the Responses backend.
                      </span>
                    )}
                  </div>
                )}

                <div className="form-group">
                  <label>Weight</label>
                  <input
                    type="number"
                    value={form.weight}
                    onChange={(e) => setForm({ ...form, weight: parseInt(e.target.value, 10) || 1 })}
                    min="1"
                    max="100"
                  />
                  <span className="form-help" style={{ fontSize: '0.8rem', opacity: 0.6 }}>
                    Routing weight (1-100) for weighted round-robin.
                  </span>
                </div>

                <div className="form-group">
                  <label>Region</label>
                  <input
                    type="text"
                    value={form.region}
                    onChange={(e) => setForm({ ...form, region: e.target.value })}
                    placeholder="us-east, eu-west"
                  />
                </div>
              </div>

              <div className="form-group form-group-inline">
                <label className="checkbox-label">
                  <input
                    type="checkbox"
                    checked={!form.disabled}
                    onChange={(e) => setForm({ ...form, disabled: !e.target.checked })}
                  />
                  Enabled
                </label>
              </div>
            </div>
            <div className="modal-footer">
              <button className="btn btn-secondary" onClick={() => setShowModal(false)}>
                Cancel
              </button>
              <button
                className="btn btn-primary"
                onClick={handleSubmit}
                disabled={saving}
              >
                {saving ? 'Saving...' : editName ? 'Update' : 'Create'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
