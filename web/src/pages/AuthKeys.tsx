import { useEffect, useState } from 'react';
import { authKeysApi } from '../services/api';
import type { AuthKey } from '../types';
import { Key, Plus, Trash2, X, Copy, Check } from 'lucide-react';

export default function AuthKeys() {
  const [keys, setKeys] = useState<AuthKey[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [showKeyModal, setShowKeyModal] = useState(false);
  const [newKeyName, setNewKeyName] = useState('');
  const [newKeyExpiry, setNewKeyExpiry] = useState('');
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

  const handleCreate = async () => {
    if (!newKeyName.trim()) {
      setError('Name is required');
      return;
    }

    setSaving(true);
    setError('');

    try {
      const response = await authKeysApi.create({
        name: newKeyName,
        expires_in_days: newKeyExpiry ? parseInt(newKeyExpiry, 10) : undefined,
      });

      setCreatedKey(response.data.key);
      setShowCreateModal(false);
      setShowKeyModal(true);
      setNewKeyName('');
      setNewKeyExpiry('');
      fetchKeys();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create key');
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async (id: string, name: string) => {
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
      // Fallback for older browsers
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

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h2>API Keys</h2>
          <p className="page-subtitle">Manage authentication keys for API access</p>
        </div>
        <button className="btn btn-primary" onClick={() => {
          setError('');
          setShowCreateModal(true);
        }}>
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
                <th>Key Prefix</th>
                <th>Created</th>
                <th>Last Used</th>
                <th>Expires</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              {isLoading ? (
                <tr>
                  <td colSpan={6} className="table-empty">Loading...</td>
                </tr>
              ) : keys.length === 0 ? (
                <tr>
                  <td colSpan={6} className="table-empty">
                    <div className="empty-state">
                      <Key size={48} />
                      <p>No API keys created</p>
                      <button className="btn btn-primary" onClick={() => setShowCreateModal(true)}>
                        <Plus size={16} />
                        Create First Key
                      </button>
                    </div>
                  </td>
                </tr>
              ) : (
                keys.map((key) => (
                  <tr key={key.id}>
                    <td className="text-bold">{key.name}</td>
                    <td className="text-mono">{key.key_prefix}...</td>
                    <td className="text-nowrap">
                      {new Date(key.created_at).toLocaleDateString()}
                    </td>
                    <td className="text-nowrap">
                      {key.last_used_at
                        ? new Date(key.last_used_at).toLocaleDateString()
                        : 'Never'}
                    </td>
                    <td className="text-nowrap">
                      {key.expires_at
                        ? new Date(key.expires_at).toLocaleDateString()
                        : 'Never'}
                    </td>
                    <td>
                      <button
                        className="btn btn-ghost btn-sm btn-danger-ghost"
                        onClick={() => handleDelete(key.id, key.name)}
                        title="Delete"
                      >
                        <Trash2 size={14} />
                      </button>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* Create Modal */}
      {showCreateModal && (
        <div className="modal-overlay" onClick={() => setShowCreateModal(false)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3>Create API Key</h3>
              <button className="btn btn-ghost btn-sm" onClick={() => setShowCreateModal(false)}>
                <X size={18} />
              </button>
            </div>
            <div className="modal-body">
              {error && <div className="form-error">{error}</div>}

              <div className="form-group">
                <label>Key Name</label>
                <input
                  type="text"
                  value={newKeyName}
                  onChange={(e) => setNewKeyName(e.target.value)}
                  placeholder="e.g., Production App"
                  autoFocus
                />
              </div>

              <div className="form-group">
                <label>Expires In (days, leave empty for no expiry)</label>
                <input
                  type="number"
                  value={newKeyExpiry}
                  onChange={(e) => setNewKeyExpiry(e.target.value)}
                  placeholder="90"
                  min="1"
                />
              </div>
            </div>
            <div className="modal-footer">
              <button className="btn btn-secondary" onClick={() => setShowCreateModal(false)}>
                Cancel
              </button>
              <button
                className="btn btn-primary"
                onClick={handleCreate}
                disabled={saving}
              >
                {saving ? 'Creating...' : 'Create Key'}
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
