import { useState } from 'react';
import { Plus, Trash2, Edit2, Check, X } from 'lucide-react';
import type { RouteRule } from '../../types';

interface RuleTableProps {
  rules: RouteRule[];
  profileNames: string[];
  onChange: (rules: RouteRule[]) => void;
}

interface EditingRule {
  name: string;
  models: string;
  tenants: string;
  useProfile: string;
}

function ruleToEditing(rule: RouteRule): EditingRule {
  return {
    name: rule.name,
    models: (rule.match.models ?? []).join(', '),
    tenants: (rule.match.tenants ?? []).join(', '),
    useProfile: rule['use-profile'],
  };
}

function editingToRule(editing: EditingRule, priority: number): RouteRule {
  return {
    name: editing.name,
    priority,
    match: {
      models: editing.models ? editing.models.split(',').map((s) => s.trim()).filter(Boolean) : undefined,
      tenants: editing.tenants ? editing.tenants.split(',').map((s) => s.trim()).filter(Boolean) : undefined,
    },
    'use-profile': editing.useProfile,
  };
}

export default function RuleTable({ rules, profileNames, onChange }: RuleTableProps) {
  const [editingIdx, setEditingIdx] = useState<number | null>(null);
  const [editingRule, setEditingRule] = useState<EditingRule | null>(null);
  const [adding, setAdding] = useState(false);
  const [newRule, setNewRule] = useState<EditingRule>({
    name: '',
    models: '',
    tenants: '',
    useProfile: profileNames[0] ?? 'balanced',
  });

  const startEdit = (idx: number) => {
    setEditingIdx(idx);
    setEditingRule(ruleToEditing(rules[idx]));
  };

  const cancelEdit = () => {
    setEditingIdx(null);
    setEditingRule(null);
  };

  const saveEdit = () => {
    if (editingIdx === null || !editingRule) return;
    const updated = [...rules];
    updated[editingIdx] = editingToRule(editingRule, editingIdx);
    onChange(updated);
    cancelEdit();
  };

  const deleteRule = (idx: number) => {
    onChange(rules.filter((_, i) => i !== idx));
  };

  const addRule = () => {
    if (!newRule.name.trim()) return;
    onChange([...rules, editingToRule(newRule, rules.length)]);
    setNewRule({ name: '', models: '', tenants: '', useProfile: profileNames[0] ?? 'balanced' });
    setAdding(false);
  };

  return (
    <div className="card">
      <div className="card-header">
        <h3>Rules</h3>
        <button className="btn btn-sm btn-secondary" onClick={() => setAdding(!adding)}>
          <Plus size={14} /> Add Rule
        </button>
      </div>
      <div className="card-body">
        {rules.length === 0 && !adding && (
          <p style={{ color: 'var(--color-text-secondary)', fontStyle: 'italic' }}>
            No rules configured. All requests use the default profile.
          </p>
        )}
        {(rules.length > 0 || adding) && (
          <table className="data-table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Model Match</th>
                <th>Tenant Match</th>
                <th>Profile</th>
                <th style={{ width: '80px' }}>Actions</th>
              </tr>
            </thead>
            <tbody>
              {rules.map((rule, idx) => (
                <tr key={idx}>
                  {editingIdx === idx && editingRule ? (
                    <>
                      <td>
                        <input
                          className="form-input form-input--sm"
                          value={editingRule.name}
                          onChange={(e) => setEditingRule({ ...editingRule, name: e.target.value })}
                        />
                      </td>
                      <td>
                        <input
                          className="form-input form-input--sm"
                          value={editingRule.models}
                          onChange={(e) => setEditingRule({ ...editingRule, models: e.target.value })}
                          placeholder="gpt-*, claude-*"
                        />
                      </td>
                      <td>
                        <input
                          className="form-input form-input--sm"
                          value={editingRule.tenants}
                          onChange={(e) => setEditingRule({ ...editingRule, tenants: e.target.value })}
                          placeholder="tenant-a, tenant-b"
                        />
                      </td>
                      <td>
                        <select
                          className="form-select form-select--sm"
                          value={editingRule.useProfile}
                          onChange={(e) => setEditingRule({ ...editingRule, useProfile: e.target.value })}
                        >
                          {profileNames.map((p) => (
                            <option key={p} value={p}>{p}</option>
                          ))}
                        </select>
                      </td>
                      <td>
                        <button className="btn-icon" onClick={saveEdit} title="Save"><Check size={14} /></button>
                        <button className="btn-icon" onClick={cancelEdit} title="Cancel"><X size={14} /></button>
                      </td>
                    </>
                  ) : (
                    <>
                      <td><code>{rule.name}</code></td>
                      <td><code>{(rule.match.models ?? []).join(', ') || '—'}</code></td>
                      <td><code>{(rule.match.tenants ?? []).join(', ') || '—'}</code></td>
                      <td><code>{rule['use-profile']}</code></td>
                      <td>
                        <button className="btn-icon" onClick={() => startEdit(idx)} title="Edit"><Edit2 size={14} /></button>
                        <button className="btn-icon btn-icon--danger" onClick={() => deleteRule(idx)} title="Delete"><Trash2 size={14} /></button>
                      </td>
                    </>
                  )}
                </tr>
              ))}
              {adding && (
                <tr>
                  <td>
                    <input
                      className="form-input form-input--sm"
                      value={newRule.name}
                      onChange={(e) => setNewRule({ ...newRule, name: e.target.value })}
                      placeholder="rule-name"
                    />
                  </td>
                  <td>
                    <input
                      className="form-input form-input--sm"
                      value={newRule.models}
                      onChange={(e) => setNewRule({ ...newRule, models: e.target.value })}
                      placeholder="gpt-*, claude-*"
                    />
                  </td>
                  <td>
                    <input
                      className="form-input form-input--sm"
                      value={newRule.tenants}
                      onChange={(e) => setNewRule({ ...newRule, tenants: e.target.value })}
                      placeholder="tenant-a, tenant-b"
                    />
                  </td>
                  <td>
                    <select
                      className="form-select form-select--sm"
                      value={newRule.useProfile}
                      onChange={(e) => setNewRule({ ...newRule, useProfile: e.target.value })}
                    >
                      {profileNames.map((p) => (
                        <option key={p} value={p}>{p}</option>
                      ))}
                    </select>
                  </td>
                  <td>
                    <button className="btn-icon" onClick={addRule} title="Add"><Check size={14} /></button>
                    <button className="btn-icon" onClick={() => setAdding(false)} title="Cancel"><X size={14} /></button>
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}
