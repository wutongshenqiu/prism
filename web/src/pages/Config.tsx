import { useEffect, useState, useCallback, useMemo } from 'react';
import { configApi } from '../services/api';
import type { ConfigSnapshot, ConfigValidateResponse } from '../types';
import {
  FileCode,
  RefreshCw,
  CheckCircle,
  XCircle,
  Eye,
  Edit3,
  Save,
  RotateCcw,
  Server,
  KeyRound,
  Shield,
  HardDrive,
  type LucideIcon,
} from 'lucide-react';

type Tab = 'view' | 'editor';

interface SummaryCard {
  label: string;
  value: string;
  subtitle: string;
  icon: LucideIcon;
}

export default function Config() {
  const [activeTab, setActiveTab] = useState<Tab>('view');
  const [currentConfig, setCurrentConfig] = useState<ConfigSnapshot | null>(null);
  const [rawYaml, setRawYaml] = useState('');
  const [configPath, setConfigPath] = useState('');
  const [editorContent, setEditorContent] = useState('');
  const [configVersion, setConfigVersion] = useState('');
  const [isLoading, setIsLoading] = useState(true);
  const [validationResult, setValidationResult] = useState<ConfigValidateResponse | null>(null);
  const [isValidating, setIsValidating] = useState(false);
  const [isApplying, setIsApplying] = useState(false);
  const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);

  const fetchConfig = useCallback(async () => {
    setIsLoading(true);
    try {
      const [currentRes, rawRes] = await Promise.all([
        configApi.current(),
        configApi.raw(),
      ]);
      setCurrentConfig(currentRes.data);
      setRawYaml(rawRes.data.content);
      setConfigPath(rawRes.data.path);
      setEditorContent(rawRes.data.content);
      setConfigVersion(rawRes.data.config_version || currentRes.data.config_version || '');
      setValidationResult(null);
    } catch (err) {
      console.error('Failed to fetch config:', err);
      setMessage({ type: 'error', text: 'Failed to load configuration' });
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchConfig();
  }, [fetchConfig]);

  const handleValidate = async () => {
    setIsValidating(true);
    setValidationResult(null);
    setMessage(null);
    try {
      const res = await configApi.validate(editorContent);
      setValidationResult(res.data);
    } catch (err) {
      if (err && typeof err === 'object' && 'response' in err) {
        const axiosErr = err as { response?: { data?: ConfigValidateResponse } };
        if (axiosErr.response?.data) {
          setValidationResult(axiosErr.response.data);
          return;
        }
      }
      setMessage({ type: 'error', text: 'Validation request failed' });
    } finally {
      setIsValidating(false);
    }
  };

  const handleApply = async () => {
    if (!window.confirm(
      'Apply this configuration? The gateway will validate, save to disk, and reload.'
    )) {
      return;
    }
    setIsApplying(true);
    setMessage(null);
    try {
      const res = await configApi.apply(editorContent, configVersion || undefined);
      if (res.data.config_version) {
        setConfigVersion(res.data.config_version);
      }
      setMessage({ type: 'success', text: 'Configuration applied successfully.' });
      await fetchConfig();
    } catch (err) {
      if (err && typeof err === 'object' && 'response' in err) {
        const axiosErr = err as {
          response?: { data?: { error?: string; message?: string; current_version?: string } };
        };
        const errorCode = axiosErr.response?.data?.error;
        const errMsg = axiosErr.response?.data?.message || 'Failed to apply configuration';
        if (errorCode === 'config_conflict') {
          if (axiosErr.response?.data?.current_version) {
            setConfigVersion(axiosErr.response.data.current_version);
          }
          setMessage({ type: 'error', text: `Conflict: ${errMsg}` });
          fetchConfig();
        } else {
          const prefix = errorCode === 'write_failed'
            ? 'Write error: '
            : errorCode === 'validation_failed'
              ? 'Validation error: '
              : '';
          setMessage({ type: 'error', text: `${prefix}${errMsg}` });
        }
      } else {
        setMessage({
          type: 'error',
          text: err instanceof Error ? err.message : 'Failed to apply configuration',
        });
      }
    } finally {
      setIsApplying(false);
    }
  };

  const handleReset = () => {
    setEditorContent(rawYaml);
    setValidationResult(null);
    setMessage(null);
  };

  const hasChanges = editorContent !== rawYaml;
  const sectionEntries = useMemo(
    () => Object.entries(currentConfig ?? {}).filter(([key]) => key !== 'config_version'),
    [currentConfig],
  );
  const summaryCards = useMemo<SummaryCard[]>(() => {
    if (!currentConfig) return [];
    return [
      {
        label: 'Sections',
        value: String(sectionEntries.length),
        subtitle: 'runtime-backed views',
        icon: FileCode,
      },
      {
        label: 'Providers',
        value: String(currentConfig.providers.total),
        subtitle: 'configured upstreams',
        icon: Server,
      },
      {
        label: 'Auth Keys',
        value: String(currentConfig.auth_keys.total),
        subtitle: 'proxy client keys',
        icon: KeyRound,
      },
      {
        label: 'Version',
        value: currentConfig.config_version.slice(0, 12),
        subtitle: 'optimistic concurrency hash',
        icon: Shield,
      },
    ];
  }, [currentConfig, sectionEntries.length]);

  if (isLoading) {
    return (
      <div className="page">
        <div className="page-header">
          <h2>Config & Changes</h2>
        </div>
        <div className="card">
          <div className="card-body">Loading configuration...</div>
        </div>
      </div>
    );
  }

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h2>Config & Changes</h2>
          <p className="page-subtitle">
            Runtime-backed config snapshot and YAML workspace
            {configPath && <> &mdash; <code>{configPath}</code></>}
          </p>
        </div>
        <div className="page-header-actions">
          <button className="btn btn-secondary" onClick={fetchConfig}>
            <RefreshCw size={16} />
            Refresh
          </button>
        </div>
      </div>

      {message && (
        <div className={`alert alert-${message.type}`} style={{ marginBottom: '1.5rem' }}>
          {message.text}
        </div>
      )}

      <div className="card" style={{ marginBottom: '1.5rem' }}>
        <div className="card-body" style={{ padding: '0.5rem 1rem' }}>
          <div style={{ display: 'flex', gap: '0.5rem' }}>
            <button
              className={`btn ${activeTab === 'view' ? 'btn-primary' : 'btn-ghost'} btn-sm`}
              onClick={() => setActiveTab('view')}
            >
              <Eye size={14} />
              Runtime Snapshot
            </button>
            <button
              className={`btn ${activeTab === 'editor' ? 'btn-primary' : 'btn-ghost'} btn-sm`}
              onClick={() => setActiveTab('editor')}
            >
              <Edit3 size={14} />
              YAML Editor
              {hasChanges && (
                <span style={{ color: 'var(--color-warning)', marginLeft: '0.25rem' }}>*</span>
              )}
            </button>
          </div>
        </div>
      </div>

      {activeTab === 'view' && currentConfig && (
        <>
          <div className="metric-grid" style={{ marginBottom: '1.5rem' }}>
            {summaryCards.map(({ label, value, subtitle, icon: Icon }) => (
              <div key={label} className="metric-card">
                <div className="metric-card-header">
                  <div className="metric-card-title">{label}</div>
                  <div className="metric-card-icon metric-card-icon--blue">
                    <Icon size={18} />
                  </div>
                </div>
                <div className="metric-card-value">{value}</div>
                <div className="metric-card-subtitle">{subtitle}</div>
              </div>
            ))}
          </div>

          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'minmax(0, 1.3fr) minmax(280px, 0.7fr)',
              gap: '1.5rem',
              marginBottom: '1.5rem',
            }}
          >
            <div className="card">
              <div className="card-header">
                <h3>Sanitized Runtime Snapshot</h3>
              </div>
              <div className="card-body">
                <pre
                  style={{
                    background: 'var(--color-bg-secondary)',
                    padding: '1rem',
                    borderRadius: '0.5rem',
                    overflow: 'auto',
                    maxHeight: '600px',
                    fontSize: '0.85rem',
                    lineHeight: '1.5',
                    margin: 0,
                  }}
                >
                  {JSON.stringify(currentConfig, null, 2)}
                </pre>
              </div>
            </div>

            <div style={{ display: 'flex', flexDirection: 'column', gap: '1.5rem' }}>
              <div className="card">
                <div className="card-header">
                  <h3>Workspace Metadata</h3>
                </div>
                <div className="card-body" style={{ display: 'grid', gap: '1rem' }}>
                  <div>
                    <div className="text-muted" style={{ fontSize: '0.75rem', marginBottom: '0.25rem' }}>
                      Config file
                    </div>
                    <div className="text-mono" style={{ fontSize: '0.85rem', wordBreak: 'break-all' }}>
                      {configPath || '-'}
                    </div>
                  </div>
                  <div>
                    <div className="text-muted" style={{ fontSize: '0.75rem', marginBottom: '0.25rem' }}>
                      Version hash
                    </div>
                    <div className="text-mono" style={{ fontSize: '0.85rem', wordBreak: 'break-all' }}>
                      {currentConfig.config_version || configVersion || '-'}
                    </div>
                  </div>
                  <div>
                    <div className="text-muted" style={{ fontSize: '0.75rem', marginBottom: '0.25rem' }}>
                      TLS / Body limit
                    </div>
                    <div>
                      {currentConfig.listen.tls_enabled ? 'TLS enabled' : 'TLS disabled'}
                      {' · '}
                      {currentConfig.listen.body_limit_mb} MB
                    </div>
                  </div>
                  <div>
                    <div className="text-muted" style={{ fontSize: '0.75rem', marginBottom: '0.25rem' }}>
                      Cache / Log store
                    </div>
                    <div>
                      {currentConfig.cache.enabled ? 'Cache on' : 'Cache off'}
                      {' · '}
                      {currentConfig.log_store.capacity} log entries
                    </div>
                  </div>
                </div>
              </div>

              <div className="card">
                <div className="card-header">
                  <h3>Runtime Sections</h3>
                </div>
                <div className="card-body">
                  <div className="tag-list">
                    {sectionEntries.map(([key]) => (
                      <span key={key} className="tag">{key}</span>
                    ))}
                  </div>
                </div>
              </div>
            </div>
          </div>

          <div className="card">
            <div className="card-header">
              <h3><HardDrive size={18} style={{ verticalAlign: 'middle', marginRight: '0.5rem' }} />Provider Inventory</h3>
            </div>
            <div className="table-wrapper">
              <table className="table">
                <thead>
                  <tr>
                    <th>Provider</th>
                    <th>Format</th>
                    <th style={{ textAlign: 'right' }}>Models</th>
                    <th>Wire API</th>
                    <th>Region</th>
                    <th>Status</th>
                  </tr>
                </thead>
                <tbody>
                  {currentConfig.providers.items.length === 0 ? (
                    <tr>
                      <td colSpan={6} className="table-empty">No providers configured</td>
                    </tr>
                  ) : (
                    currentConfig.providers.items.map((provider) => (
                      <tr key={provider.name}>
                        <td className="text-bold">{provider.name}</td>
                        <td><span className="type-badge">{provider.format}</span></td>
                        <td style={{ textAlign: 'right' }}>{provider.models_count}</td>
                        <td><span className="type-badge">{provider.wire_api}</span></td>
                        <td>{provider.region || <span className="text-muted">global</span>}</td>
                        <td>
                          <span className={`type-badge ${provider.disabled ? 'type-badge--red' : 'type-badge--green'}`}>
                            {provider.disabled ? 'disabled' : 'active'}
                          </span>
                        </td>
                      </tr>
                    ))
                  )}
                </tbody>
              </table>
            </div>
          </div>
        </>
      )}

      {activeTab === 'editor' && (
        <>
          <div className="card">
            <div className="card-header card-header--actions">
              <h3>
                <FileCode size={18} style={{ verticalAlign: 'middle', marginRight: '0.5rem' }} />
                YAML Configuration
                {hasChanges && (
                  <span
                    style={{
                      color: 'var(--color-warning)',
                      fontSize: '0.85rem',
                      marginLeft: '0.5rem',
                    }}
                  >
                    (unsaved changes)
                  </span>
                )}
              </h3>
              <div style={{ display: 'flex', gap: '0.5rem' }}>
                {hasChanges && (
                  <button className="btn btn-ghost btn-sm" onClick={handleReset}>
                    <RotateCcw size={14} />
                    Reset
                  </button>
                )}
                <button
                  className="btn btn-secondary btn-sm"
                  onClick={handleValidate}
                  disabled={isValidating}
                >
                  <CheckCircle size={14} />
                  {isValidating ? 'Validating...' : 'Validate'}
                </button>
                <button
                  className="btn btn-primary btn-sm"
                  onClick={handleApply}
                  disabled={isApplying || !hasChanges}
                  title={!hasChanges ? 'No changes to apply' : 'Validate, save to disk, and reload'}
                >
                  <Save size={14} />
                  {isApplying ? 'Applying...' : 'Save & Apply'}
                </button>
              </div>
            </div>
            <div className="card-body" style={{ padding: 0 }}>
              <textarea
                value={editorContent}
                onChange={(e) => {
                  setEditorContent(e.target.value);
                  setValidationResult(null);
                }}
                spellCheck={false}
                style={{
                  width: '100%',
                  minHeight: '500px',
                  fontFamily: 'monospace',
                  fontSize: '0.85rem',
                  lineHeight: '1.6',
                  padding: '1rem',
                  border: 'none',
                  outline: 'none',
                  resize: 'vertical',
                  background: 'var(--color-bg-secondary)',
                  color: 'var(--color-text)',
                  tabSize: 2,
                }}
              />
            </div>
          </div>

          {validationResult && (
            <div
              className={`alert alert-${validationResult.valid ? 'success' : 'error'}`}
              style={{ marginTop: '1rem' }}
            >
              <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
                {validationResult.valid ? (
                  <>
                    <CheckCircle size={16} />
                    Configuration is valid
                  </>
                ) : (
                  <>
                    <XCircle size={16} />
                    Configuration has errors:
                  </>
                )}
              </div>
              {!validationResult.valid && validationResult.errors.length > 0 && (
                <ul style={{ margin: '0.5rem 0 0 1.5rem', padding: 0 }}>
                  {validationResult.errors.map((err, i) => (
                    <li key={i} style={{ marginBottom: '0.25rem' }}>{err}</li>
                  ))}
                </ul>
              )}
            </div>
          )}
        </>
      )}
    </div>
  );
}
