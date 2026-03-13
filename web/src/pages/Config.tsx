import { useEffect, useState, useCallback } from 'react';
import { configApi } from '../services/api';
import type { ConfigValidateResponse } from '../types';
import {
  FileCode,
  RefreshCw,
  CheckCircle,
  XCircle,
  Eye,
  Edit3,
  Save,
  RotateCcw,
} from 'lucide-react';

type Tab = 'view' | 'editor';

export default function Config() {
  const [activeTab, setActiveTab] = useState<Tab>('view');
  const [currentConfig, setCurrentConfig] = useState<Record<string, unknown> | null>(null);
  const [rawYaml, setRawYaml] = useState('');
  const [configPath, setConfigPath] = useState('');
  const [editorContent, setEditorContent] = useState('');
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
      await configApi.apply(editorContent);
      setMessage({ type: 'success', text: 'Configuration applied successfully.' });
      await fetchConfig();
    } catch (err) {
      if (err && typeof err === 'object' && 'response' in err) {
        const axiosErr = err as { response?: { data?: { message?: string } } };
        const errMsg = axiosErr.response?.data?.message || 'Failed to apply configuration';
        setMessage({ type: 'error', text: errMsg });
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

  if (isLoading) {
    return (
      <div className="page">
        <div className="page-header">
          <h2>Configuration</h2>
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
          <h2>Configuration</h2>
          <p className="page-subtitle">
            View and manage gateway configuration
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

      {/* Tab navigation */}
      <div className="card" style={{ marginBottom: '1.5rem' }}>
        <div className="card-body" style={{ padding: '0.5rem 1rem' }}>
          <div style={{ display: 'flex', gap: '0.5rem' }}>
            <button
              className={`btn ${activeTab === 'view' ? 'btn-primary' : 'btn-ghost'} btn-sm`}
              onClick={() => setActiveTab('view')}
            >
              <Eye size={14} />
              Current Config
            </button>
            <button
              className={`btn ${activeTab === 'editor' ? 'btn-primary' : 'btn-ghost'} btn-sm`}
              onClick={() => setActiveTab('editor')}
            >
              <Edit3 size={14} />
              YAML Editor
              {hasChanges && <span style={{ color: 'var(--warning)', marginLeft: '0.25rem' }}>*</span>}
            </button>
          </div>
        </div>
      </div>

      {activeTab === 'view' && currentConfig && (
        <div className="card">
          <div className="card-header">
            <h3>Current Configuration (Sanitized)</h3>
          </div>
          <div className="card-body">
            <pre style={{
              background: 'var(--bg-secondary)',
              padding: '1rem',
              borderRadius: '0.5rem',
              overflow: 'auto',
              maxHeight: '600px',
              fontSize: '0.85rem',
              lineHeight: '1.5',
            }}>
              {JSON.stringify(currentConfig, null, 2)}
            </pre>
          </div>
        </div>
      )}

      {activeTab === 'editor' && (
        <>
          <div className="card">
            <div className="card-header card-header--actions">
              <h3>
                <FileCode size={18} style={{ verticalAlign: 'middle', marginRight: '0.5rem' }} />
                YAML Configuration
                {hasChanges && (
                  <span style={{ color: 'var(--warning)', fontSize: '0.85rem', marginLeft: '0.5rem' }}>
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
                  background: 'var(--bg-secondary)',
                  color: 'var(--text-primary)',
                  tabSize: 2,
                }}
              />
            </div>
          </div>

          {/* Validation result */}
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
