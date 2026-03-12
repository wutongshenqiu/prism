import { useEffect, useState } from 'react';
import { providersApi } from '../services/api';
import type { Provider, ProviderCreateRequest, ProviderType } from '../types';
import StatusBadge from '../components/StatusBadge';
import TagList from '../components/TagList';
import { Server, Plus, Pencil, Trash2, X, RefreshCw, HeartPulse, PlusCircle, MinusCircle, Copy } from 'lucide-react';

const PROVIDER_TYPES: { value: ProviderType; label: string }[] = [
  { value: 'openai', label: 'OpenAI' },
  { value: 'claude', label: 'Claude (Anthropic)' },
  { value: 'gemini', label: 'Gemini (Google)' },
  { value: 'openai_compat', label: 'OpenAI Compatible' },
];

const DEFAULT_BASE_URLS: Record<ProviderType, string> = {
  openai: 'https://api.openai.com/v1',
  claude: 'https://api.anthropic.com/v1',
  gemini: 'https://generativelanguage.googleapis.com/v1beta',
  openai_compat: '',
};

interface HeaderPair {
  key: string;
  value: string;
}

interface FormState {
  name: string;
  provider_type: ProviderType;
  base_url: string;
  api_key: string;
  enabled: boolean;
  models: string;
  headers: HeaderPair[];
}

const emptyForm: FormState = {
  name: '',
  provider_type: 'openai',
  base_url: DEFAULT_BASE_URLS.openai,
  api_key: '',
  enabled: true,
  models: '',
  headers: [],
};

export default function Providers() {
  const [providers, setProviders] = useState<Provider[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [showModal, setShowModal] = useState(false);
  const [editId, setEditId] = useState<string | null>(null);
  const [form, setForm] = useState<FormState>(emptyForm);
  const [error, setError] = useState('');
  const [saving, setSaving] = useState(false);
  const [fetchingModels, setFetchingModels] = useState(false);
  const [healthChecking, setHealthChecking] = useState<string | null>(null);
  const [healthResults, setHealthResults] = useState<Record<string, { status: string; latency_ms?: number; message?: string }>>({});

  const fetchProviders = async () => {
    try {
      const response = await providersApi.list();
      setProviders(response.data);
    } catch (err) {
      console.error('Failed to fetch providers:', err);
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    fetchProviders();
  }, []);

  const openCreate = () => {
    setEditId(null);
    setForm(emptyForm);
    setError('');
    setShowModal(true);
  };

  const openEdit = (provider: Provider) => {
    setEditId(provider.id);
    const headerPairs: HeaderPair[] = provider.headers
      ? Object.entries(provider.headers).map(([key, value]) => ({ key, value }))
      : [];
    setForm({
      name: provider.name ?? '',
      provider_type: provider.provider_type,
      base_url: provider.base_url ?? '',
      api_key: '',
      enabled: provider.enabled,
      models: provider.models.join(', '),
      headers: headerPairs,
    });
    setError('');
    setShowModal(true);
  };

  const handleSubmit = async () => {
    if (!form.name.trim()) {
      setError('Name is required');
      return;
    }
    if (!form.base_url.trim()) {
      setError('Base URL is required');
      return;
    }
    if (!editId && !form.api_key.trim()) {
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

      const headers: Record<string, string> = {};
      form.headers.forEach(({ key, value }) => {
        if (key.trim() && value.trim()) headers[key.trim()] = value.trim();
      });

      if (editId) {
        await providersApi.update(editId, {
          name: form.name,
          base_url: form.base_url,
          api_key: form.api_key || undefined,
          enabled: form.enabled,
          models,
          headers,
        });
      } else {
        const data: ProviderCreateRequest = {
          name: form.name,
          provider_type: form.provider_type,
          base_url: form.base_url,
          api_key: form.api_key,
          enabled: form.enabled,
          models,
          headers,
        };
        await providersApi.create(data);
      }

      setShowModal(false);
      fetchProviders();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to save provider');
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async (id: string, name: string) => {
    if (!window.confirm(`Delete provider "${name}"? This cannot be undone.`)) {
      return;
    }

    try {
      await providersApi.delete(id);
      fetchProviders();
    } catch (err) {
      console.error('Failed to delete provider:', err);
    }
  };

  const handleTypeChange = (type: ProviderType) => {
    setForm((prev) => ({
      ...prev,
      provider_type: type,
      base_url: prev.base_url === DEFAULT_BASE_URLS[prev.provider_type]
        ? DEFAULT_BASE_URLS[type]
        : prev.base_url,
    }));
  };

  const handleFetchModels = async () => {
    setFetchingModels(true);
    try {
      const result = await providersApi.fetchModels({
        provider_type: form.provider_type,
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

  const handleHealthCheck = async (id: string) => {
    setHealthChecking(id);
    try {
      const result = await providersApi.healthCheck(id);
      setHealthResults((prev) => ({ ...prev, [id]: result }));
    } catch (err) {
      setHealthResults((prev) => ({
        ...prev,
        [id]: { status: 'error', message: err instanceof Error ? err.message : 'Health check failed' },
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

      <div className="card">
        <div className="table-wrapper">
          <table className="table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Type</th>
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
                  <tr key={provider.id}>
                    <td className="text-bold">{provider.name}</td>
                    <td>
                      <span className="type-badge">
                        {PROVIDER_TYPES.find((t) => t.value === provider.provider_type)?.label ?? provider.provider_type}
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
                        <TagList items={provider.models} maxVisible={3} />
                      ) : provider.models_count != null ? (
                        <div className="tag-list">
                          <span className="tag">{provider.models_count} models</span>
                        </div>
                      ) : (
                        <span className="text-muted">-</span>
                      )}
                    </td>
                    <td>
                      {healthResults[provider.id] ? (
                        <span
                          className={`health-badge ${healthResults[provider.id].status === 'ok' ? 'health-ok' : 'health-error'}`}
                          title={healthResults[provider.id].message || `${healthResults[provider.id].latency_ms}ms`}
                        >
                          {healthResults[provider.id].status === 'ok'
                            ? `✓ ${healthResults[provider.id].latency_ms}ms`
                            : '✗ Error'}
                        </span>
                      ) : (
                        <StatusBadge
                          status={provider.enabled ? 'active' : 'inactive'}
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
                          onClick={() => handleHealthCheck(provider.id)}
                          title="Health Check"
                          disabled={healthChecking === provider.id}
                        >
                          <HeartPulse size={14} />
                        </button>
                        <button
                          className="btn btn-ghost btn-sm btn-danger-ghost"
                          onClick={() => handleDelete(provider.id, provider.name ?? 'Unnamed')}
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
              <h3>{editId ? 'Edit Provider' : 'Add Provider'}</h3>
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
                  placeholder="e.g., OpenAI Production"
                />
              </div>

              <div className="form-group">
                <label>Provider Type</label>
                <select
                  value={form.provider_type}
                  onChange={(e) => handleTypeChange(e.target.value as ProviderType)}
                  disabled={!!editId}
                >
                  {PROVIDER_TYPES.map((t) => (
                    <option key={t.value} value={t.value}>{t.label}</option>
                  ))}
                </select>
              </div>

              <div className="form-group">
                <label>Base URL</label>
                <input
                  type="text"
                  value={form.base_url}
                  onChange={(e) => setForm({ ...form, base_url: e.target.value })}
                  placeholder="https://api.example.com/v1"
                />
              </div>

              <div className="form-group">
                <label>API Key {editId && '(leave empty to keep current)'}</label>
                <input
                  type="password"
                  value={form.api_key}
                  onChange={(e) => setForm({ ...form, api_key: e.target.value })}
                  placeholder={editId ? '********' : 'sk-...'}
                />
              </div>

              <div className="form-group">
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                  <label>Models (comma-separated)</label>
                  <button
                    type="button"
                    className="btn btn-ghost btn-sm"
                    onClick={handleFetchModels}
                    disabled={fetchingModels || (!form.api_key && !editId)}
                    title={editId && !form.api_key ? 'Enter API key to fetch models' : 'Fetch available models from provider'}
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

              <div className="form-group form-group-inline">
                <label className="checkbox-label">
                  <input
                    type="checkbox"
                    checked={form.enabled}
                    onChange={(e) => setForm({ ...form, enabled: e.target.checked })}
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
                {saving ? 'Saving...' : editId ? 'Update' : 'Create'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
