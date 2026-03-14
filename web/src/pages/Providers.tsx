import { useEffect, useState } from 'react';
import { providersApi } from '../services/api';
import type { Provider, ProviderCreateRequest, FormatType } from '../types';
import StatusBadge from '../components/StatusBadge';
import TagList from '../components/TagList';
import { Server, Plus, Pencil, Trash2, X, RefreshCw, HeartPulse, PlusCircle, MinusCircle, Copy } from 'lucide-react';

const FORMAT_OPTIONS: { value: FormatType; label: string }[] = [
  { value: 'openai', label: 'OpenAI' },
  { value: 'claude', label: 'Claude (Anthropic)' },
  { value: 'gemini', label: 'Gemini (Google)' },
];

const DEFAULT_BASE_URLS: Record<FormatType, string> = {
  openai: 'https://api.openai.com',
  claude: 'https://api.anthropic.com',
  gemini: 'https://generativelanguage.googleapis.com',
};

interface HeaderPair {
  key: string;
  value: string;
}

interface FormState {
  name: string;
  format: FormatType;
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
}

interface NoticeState {
  type: 'success' | 'error' | 'warning';
  message: string;
}

const emptyForm: FormState = {
  name: '',
  format: 'openai',
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
};

export default function Providers() {
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
  const [healthResults, setHealthResults] = useState<Record<string, { status: string; latency_ms?: number; message?: string }>>({});

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
    setError('');
    setShowModal(true);
  };

  const openEdit = (provider: Provider) => {
    setEditName(provider.name);
    const headerPairs: HeaderPair[] = provider.headers
      ? Object.entries(provider.headers).map(([key, value]) => ({ key, value }))
      : [];
    const modelStrings = (provider.models || []).map((m) =>
      typeof m === 'string' ? m : m.id
    );
    setForm({
      name: provider.name,
      format: provider.format,
      base_url: provider.base_url ?? '',
      proxy_url: provider.proxy_url ?? '',
      api_key: '',
      prefix: provider.prefix ?? '',
      disabled: provider.disabled,
      models: modelStrings.join(', '),
      excluded_models: (provider.excluded_models || []).join(', '),
      headers: headerPairs,
      wire_api: provider.wire_api ?? 'chat',
      weight: provider.weight ?? 1,
      region: provider.region ?? '',
    });
    setError('');
    setShowModal(true);
  };

  const handleSubmit = async () => {
    const providerName = form.name.trim();
    if (!providerName) {
      setError('Name is required');
      return;
    }
    if (!editName && !form.api_key.trim()) {
      setError('API key is required');
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

      if (editName) {
        await providersApi.update(editName, {
          base_url: form.base_url || null,
          proxy_url: form.proxy_url || null,
          api_key: form.api_key || undefined,
          prefix: form.prefix || null,
          disabled: form.disabled,
          models,
          excluded_models,
          headers,
          wire_api: form.wire_api,
          weight: form.weight,
          region: form.region || null,
        });
      } else {
        const data: ProviderCreateRequest = {
          name: providerName,
          format: form.format,
          base_url: form.base_url || undefined,
          proxy_url: form.proxy_url || undefined,
          api_key: form.api_key,
          prefix: form.prefix || undefined,
          disabled: form.disabled,
          models,
          excluded_models,
          headers,
          wire_api: form.wire_api,
          weight: form.weight,
          region: form.region || undefined,
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
    setForm((prev) => ({
      ...prev,
      format: fmt,
      base_url: prev.base_url === DEFAULT_BASE_URLS[prev.format]
        ? DEFAULT_BASE_URLS[fmt]
        : prev.base_url,
    }));
  };

  const handleFetchModels = async () => {
    setFetchingModels(true);
    try {
      const result = await providersApi.fetchModels({
        format: form.format,
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
        [name]: { status: 'error', message: err instanceof Error ? err.message : 'Health check failed' },
      }));
    } finally {
      setHealthChecking(null);
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
                <th>Format</th>
                <th>Base URL</th>
                <th>Models</th>
                <th>Status</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              {isLoading ? (
                <tr>
                  <td colSpan={6} className="table-empty">Loading...</td>
                </tr>
              ) : providers.length === 0 ? (
                <tr>
                  <td colSpan={6} className="table-empty">
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
                    <td className="text-bold">{provider.name}</td>
                    <td>
                      <span className="type-badge">
                        {FORMAT_OPTIONS.find((t) => t.value === provider.format)?.label ?? provider.format}
                      </span>
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
                      {(provider.models || []).length > 0 ? (
                        <TagList items={(provider.models || []).map((m) => typeof m === 'string' ? m : m.id)} maxVisible={3} />
                      ) : provider.models_count != null ? (
                        <div className="tag-list">
                          <span className="tag">{provider.models_count} models</span>
                        </div>
                      ) : (
                        <span className="text-muted">-</span>
                      )}
                    </td>
                    <td>
                      {healthResults[provider.name] ? (
                        <span
                          className={`health-badge ${healthResults[provider.name].status === 'ok' ? 'health-ok' : 'health-error'}`}
                          title={healthResults[provider.name].message || `${healthResults[provider.name].latency_ms}ms`}
                        >
                          {healthResults[provider.name].status === 'ok'
                            ? `✓ ${healthResults[provider.name].latency_ms}ms`
                            : '✗ Error'}
                        </span>
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
                <label>Base URL</label>
                <input
                  type="text"
                  value={form.base_url}
                  onChange={(e) => setForm({ ...form, base_url: e.target.value })}
                  placeholder={DEFAULT_BASE_URLS[form.format]}
                />
              </div>

              <div className="form-group">
                <label>API Key {editName && '(leave empty to keep current)'}</label>
                <input
                  type="password"
                  value={form.api_key}
                  onChange={(e) => setForm({ ...form, api_key: e.target.value })}
                  placeholder={editName ? '********' : 'sk-...'}
                />
              </div>

              <div className="form-group">
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                  <label>Models (comma-separated)</label>
                  <button
                    type="button"
                    className="btn btn-ghost btn-sm"
                    onClick={handleFetchModels}
                    disabled={fetchingModels || (!form.api_key && !editName)}
                    title={editName && !form.api_key ? 'Enter API key to fetch models' : 'Fetch available models from provider'}
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

              <div className="form-group">
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                  <label>Custom Headers</label>
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
                    No custom headers. Click "Add Header" to configure User-Agent, etc.
                  </p>
                )}
              </div>

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
                    >
                      <option value="chat">Chat Completions</option>
                      <option value="responses">Responses API</option>
                    </select>
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
