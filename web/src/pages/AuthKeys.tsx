import { useEffect, useState } from 'react';
import { authKeysApi } from '../services/api';
import type { AuthKey, AuthKeyCreateRequest, AuthKeyUpdateRequest, KeyRateLimitConfig, BudgetConfig } from '../types';
import { Key, Plus, Pencil, Trash2, X, Copy, Check } from 'lucide-react';
import TagList from '../components/TagList';

interface FormState {
  name: string;
  tenant_id: string;
  allowed_models: string;
  allowed_credentials: string;
  expires_days: string;
  rpm: string;
  tpm: string;
  cost_per_day_usd: string;
  budget_enabled: boolean;
  budget_total_usd: string;
  budget_period: 'daily' | 'monthly';
}

const emptyForm: FormState = {
  name: '',
  tenant_id: '',
  allowed_models: '',
  allowed_credentials: '',
  expires_days: '',
  rpm: '',
  tpm: '',
  cost_per_day_usd: '',
  budget_enabled: false,
  budget_total_usd: '',
  budget_period: 'daily',
};

function formFromKey(key: AuthKey): FormState {
  return {
    name: key.name ?? '',
    tenant_id: key.tenant_id ?? '',
    allowed_models: key.allowed_models.join(', '),
    allowed_credentials: key.allowed_credentials.join(', '),
    expires_days: '',
    rpm: key.rate_limit?.rpm?.toString() ?? '',
    tpm: key.rate_limit?.tpm?.toString() ?? '',
    cost_per_day_usd: key.rate_limit?.cost_per_day_usd?.toString() ?? '',
    budget_enabled: key.budget != null,
    budget_total_usd: key.budget?.total_usd?.toString() ?? '',
    budget_period: key.budget?.period ?? 'daily',
  };
}

function parseListField(value: string): string[] {
  return value.split(',').map((s) => s.trim()).filter(Boolean);
}

function buildRateLimit(form: FormState): KeyRateLimitConfig | undefined {
  const rpm = form.rpm ? parseInt(form.rpm, 10) : undefined;
  const tpm = form.tpm ? parseInt(form.tpm, 10) : undefined;
  const cost = form.cost_per_day_usd ? parseFloat(form.cost_per_day_usd) : undefined;
  if (rpm === undefined && tpm === undefined && cost === undefined) return undefined;
  return { rpm, tpm, cost_per_day_usd: cost };
}

function buildBudget(form: FormState): BudgetConfig | undefined {
  if (!form.budget_enabled || !form.budget_total_usd) return undefined;
  return { total_usd: parseFloat(form.budget_total_usd), period: form.budget_period };
}

export default function AuthKeys() {
  const [keys, setKeys] = useState<AuthKey[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [showModal, setShowModal] = useState(false);
  const [editId, setEditId] = useState<number | null>(null);
  const [showKeyModal, setShowKeyModal] = useState(false);
  const [form, setForm] = useState<FormState>(emptyForm);
  const [createdKey, setCreatedKey] = useState('');
  const [error, setError] = useState('');
  const [saving, setSaving] = useState(false);
  const [copied, setCopied] = useState(false);

  const fetchKeys = async () => {
    try {
      const response = await authKeysApi.list();
      setKeys(response.data);
    } catch (err) {
      console.error('Failed to fetch auth keys:', err);
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    fetchKeys();
  }, []);

  const openCreate = () => {
    setEditId(null);
    setForm(emptyForm);
    setError('');
    setShowModal(true);
  };

  const openEdit = (key: AuthKey) => {
    setEditId(key.id);
    setForm(formFromKey(key));
    setError('');
    setShowModal(true);
  };

  const handleSubmit = async () => {
    if (!editId && !form.name.trim()) {
      setError('Name is required');
      return;
    }

    setSaving(true);
    setError('');

    try {
      const models = parseListField(form.allowed_models);
      const credentials = parseListField(form.allowed_credentials);
      const rateLimit = buildRateLimit(form);
      const budget = buildBudget(form);

      if (editId !== null) {
        const data: AuthKeyUpdateRequest = {
          name: form.name || undefined,
          tenant_id: form.tenant_id || null,
          allowed_models: models,
          allowed_credentials: credentials,
          rate_limit: rateLimit ?? null,
          budget: budget ?? null,
        };
        await authKeysApi.update(editId, data);
        setShowModal(false);
      } else {
        const data: AuthKeyCreateRequest = {
          name: form.name || undefined,
          tenant_id: form.tenant_id || undefined,
          allowed_models: models.length > 0 ? models : undefined,
          allowed_credentials: credentials.length > 0 ? credentials : undefined,
          rate_limit: rateLimit,
          budget: budget,
          expires_at: form.expires_days
            ? new Date(Date.now() + parseInt(form.expires_days, 10) * 86400000).toISOString()
            : undefined,
        };

        const response = await authKeysApi.create(data);
        setCreatedKey(response.data.key);
        setShowModal(false);
        setShowKeyModal(true);
      }

      setForm(emptyForm);
      fetchKeys();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to save key');
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async (id: number, name: string) => {
    if (!window.confirm(`Delete API key "${name}"? This cannot be undone.`)) {
      return;
    }

    try {
      await authKeysApi.delete(id);
      fetchKeys();
    } catch (err) {
      console.error('Failed to delete key:', err);
    }
  };

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(createdKey);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      const textarea = document.createElement('textarea');
      textarea.value = createdKey;
      document.body.appendChild(textarea);
      textarea.select();
      document.execCommand('copy');
      document.body.removeChild(textarea);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  const renderLimits = (key: AuthKey) => {
    const parts: string[] = [];
    if (key.rate_limit?.rpm) parts.push(`${key.rate_limit.rpm} RPM`);
    if (key.rate_limit?.tpm) parts.push(`${key.rate_limit.tpm} TPM`);
    if (key.rate_limit?.cost_per_day_usd) parts.push(`$${key.rate_limit.cost_per_day_usd}/day`);
    if (key.budget) parts.push(`$${key.budget.total_usd} ${key.budget.period}`);
    return parts.length > 0 ? parts.join(', ') : '-';
  };

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h2>API Keys</h2>
          <p className="page-subtitle">Manage authentication keys for API access</p>
        </div>
        <button className="btn btn-primary" onClick={openCreate}>
          <Plus size={16} />
          Create Key
        </button>
      </div>

      <div className="card">
        <div className="table-wrapper">
          <table className="table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Key</th>
                <th className="hide-mobile">Tenant</th>
                <th>Models</th>
                <th className="hide-mobile">Credentials</th>
                <th className="hide-mobile">Limits</th>
                <th>Expires</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              {isLoading ? (
                <tr>
                  <td colSpan={8} className="table-empty">Loading...</td>
                </tr>
              ) : keys.length === 0 ? (
                <tr>
                  <td colSpan={8} className="table-empty">
                    <div className="empty-state">
                      <Key size={48} />
                      <p>No API keys created</p>
                      <button className="btn btn-primary" onClick={openCreate}>
                        <Plus size={16} />
                        Create First Key
                      </button>
                    </div>
                  </td>
                </tr>
              ) : (
                keys.map((key) => (
                  <tr key={key.id}>
                    <td className="text-bold">{key.name ?? '-'}</td>
                    <td className="text-mono">{key.key_masked || '-'}</td>
                    <td className="hide-mobile">{key.tenant_id ?? '-'}</td>
                    <td>
                      <TagList items={key.allowed_models} maxVisible={2} />
                    </td>
                    <td className="hide-mobile">
                      <TagList items={key.allowed_credentials} maxVisible={2} />
                    </td>
                    <td className="text-muted hide-mobile">{renderLimits(key)}</td>
                    <td className="text-nowrap">
                      {key.expires_at
                        ? new Date(key.expires_at).toLocaleDateString()
                        : 'Never'}
                    </td>
                    <td>
                      <div className="action-btns">
                        <button
                          className="btn btn-ghost btn-sm"
                          onClick={() => openEdit(key)}
                          title="Edit"
                        >
                          <Pencil size={14} />
                        </button>
                        <button
                          className="btn btn-ghost btn-sm btn-danger-ghost"
                          onClick={() => handleDelete(key.id, key.name ?? 'Unnamed')}
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

      {/* Create / Edit Modal */}
      {showModal && (
        <div className="modal-overlay" onClick={() => setShowModal(false)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3>{editId !== null ? 'Edit API Key' : 'Create API Key'}</h3>
              <button className="btn btn-ghost btn-sm" onClick={() => setShowModal(false)}>
                <X size={18} />
              </button>
            </div>
            <div className="modal-body">
              {error && <div className="form-error">{error}</div>}

              <div className="form-row">
                <div className="form-group">
                  <label>Key Name</label>
                  <input
                    type="text"
                    value={form.name}
                    onChange={(e) => setForm({ ...form, name: e.target.value })}
                    placeholder="e.g., Production App"
                    autoFocus
                  />
                </div>
                <div className="form-group">
                  <label>Tenant ID</label>
                  <input
                    type="text"
                    value={form.tenant_id}
                    onChange={(e) => setForm({ ...form, tenant_id: e.target.value })}
                    placeholder="Optional"
                  />
                </div>
              </div>

              {editId === null && (
                <div className="form-group">
                  <label>Expires In (days, leave empty for no expiry)</label>
                  <input
                    type="number"
                    value={form.expires_days}
                    onChange={(e) => setForm({ ...form, expires_days: e.target.value })}
                    placeholder="90"
                    min="1"
                  />
                </div>
              )}

              <div className="form-group">
                <label>Allowed Models</label>
                <input
                  type="text"
                  value={form.allowed_models}
                  onChange={(e) => setForm({ ...form, allowed_models: e.target.value })}
                  placeholder="gpt-4o, claude-*, gemini-* (empty = all)"
                />
                <span className="form-help">Comma-separated. Supports glob patterns (* wildcard). Empty allows all models.</span>
              </div>

              <div className="form-group">
                <label>Allowed Credentials</label>
                <input
                  type="text"
                  value={form.allowed_credentials}
                  onChange={(e) => setForm({ ...form, allowed_credentials: e.target.value })}
                  placeholder="prod-openai-*, my-claude-key (empty = all)"
                />
                <span className="form-help">Comma-separated credential names. Supports glob patterns. Empty allows all credentials.</span>
              </div>

              <div className="modal-section">
                <label className="modal-section-title">Rate Limits</label>
                <div className="form-row-3">
                  <div className="form-group">
                    <label>RPM</label>
                    <input
                      type="number"
                      value={form.rpm}
                      onChange={(e) => setForm({ ...form, rpm: e.target.value })}
                      placeholder="No limit"
                      min="0"
                    />
                  </div>
                  <div className="form-group">
                    <label>TPM</label>
                    <input
                      type="number"
                      value={form.tpm}
                      onChange={(e) => setForm({ ...form, tpm: e.target.value })}
                      placeholder="No limit"
                      min="0"
                    />
                  </div>
                  <div className="form-group">
                    <label>Cost/Day (USD)</label>
                    <input
                      type="number"
                      value={form.cost_per_day_usd}
                      onChange={(e) => setForm({ ...form, cost_per_day_usd: e.target.value })}
                      placeholder="No limit"
                      min="0"
                      step="0.01"
                    />
                  </div>
                </div>
              </div>

              <div className="modal-section">
                <div className="form-group form-group-inline">
                  <label className="checkbox-label">
                    <input
                      type="checkbox"
                      checked={form.budget_enabled}
                      onChange={(e) => setForm({ ...form, budget_enabled: e.target.checked })}
                    />
                    Enable Budget
                  </label>
                </div>
                {form.budget_enabled && (
                  <div className="form-row">
                    <div className="form-group">
                      <label>Total Budget (USD)</label>
                      <input
                        type="number"
                        value={form.budget_total_usd}
                        onChange={(e) => setForm({ ...form, budget_total_usd: e.target.value })}
                        placeholder="100.00"
                        min="0"
                        step="0.01"
                      />
                    </div>
                    <div className="form-group">
                      <label>Period</label>
                      <select
                        value={form.budget_period}
                        onChange={(e) => setForm({ ...form, budget_period: e.target.value as 'daily' | 'monthly' })}
                      >
                        <option value="daily">Daily</option>
                        <option value="monthly">Monthly</option>
                      </select>
                    </div>
                  </div>
                )}
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
                {saving ? 'Saving...' : editId !== null ? 'Update' : 'Create Key'}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Key Display Modal */}
      {showKeyModal && (
        <div className="modal-overlay">
          <div className="modal">
            <div className="modal-header">
              <h3>API Key Created</h3>
            </div>
            <div className="modal-body">
              <div className="alert alert-warning">
                Copy this key now. You will not be able to see it again.
              </div>
              <div className="key-display">
                <code>{createdKey}</code>
                <button
                  className="btn btn-ghost btn-sm"
                  onClick={handleCopy}
                  title="Copy to clipboard"
                >
                  {copied ? <Check size={16} /> : <Copy size={16} />}
                </button>
              </div>
            </div>
            <div className="modal-footer">
              <button
                className="btn btn-primary"
                onClick={() => {
                  setShowKeyModal(false);
                  setCreatedKey('');
                  setCopied(false);
                }}
              >
                Done
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
