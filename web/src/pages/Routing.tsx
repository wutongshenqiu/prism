import { useEffect, useState } from 'react';
import { routingApi } from '../services/api';
import type { RoutingConfig, RoutingStrategy } from '../types';
import { GitBranch, Save, RotateCcw, Plus, Trash2 } from 'lucide-react';

const STRATEGIES: { value: RoutingStrategy; label: string; description: string }[] = [
  {
    value: 'round-robin',
    label: 'Round Robin',
    description: 'Distribute requests evenly across all active credentials in sequence.',
  },
  {
    value: 'fill-first',
    label: 'Fill First',
    description: 'Prioritize the first credential by weight, only use others when exhausted.',
  },
  {
    value: 'latency-aware',
    label: 'Latency Aware',
    description: 'Route to the credential with the lowest average response time.',
  },
  {
    value: 'geo-aware',
    label: 'Geo Aware',
    description: 'Route based on client region matching credential region tags.',
  },
];

interface ModelStrategyRow {
  pattern: string;
  strategy: RoutingStrategy;
}

interface ModelFallbackRow {
  pattern: string;
  fallbacks: string;
}

export default function Routing() {
  const [config, setConfig] = useState<RoutingConfig | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState('');

  // Editable state
  const [strategy, setStrategy] = useState<RoutingStrategy>('round-robin');
  const [fallbackEnabled, setFallbackEnabled] = useState(true);
  const [requestRetry, setRequestRetry] = useState(3);
  const [maxRetryInterval, setMaxRetryInterval] = useState(30);
  const [modelStrategies, setModelStrategies] = useState<ModelStrategyRow[]>([]);
  const [modelFallbacks, setModelFallbacks] = useState<ModelFallbackRow[]>([]);

  const loadConfig = (data: RoutingConfig) => {
    setConfig(data);
    setStrategy(data.strategy);
    setFallbackEnabled(data.fallback_enabled);
    setRequestRetry(data.request_retry);
    setMaxRetryInterval(data.max_retry_interval);
    setModelStrategies(
      Object.entries(data.model_strategies).map(([pattern, strategy]) => ({ pattern, strategy }))
    );
    setModelFallbacks(
      Object.entries(data.model_fallbacks).map(([pattern, fallbacks]) => ({
        pattern,
        fallbacks: fallbacks.join(', '),
      }))
    );
  };

  const fetchConfig = async () => {
    try {
      const response = await routingApi.get();
      loadConfig(response.data);
    } catch (err) {
      console.error('Failed to fetch routing config:', err);
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    fetchConfig();
  }, []);

  const buildPayload = () => {
    const ms: Record<string, RoutingStrategy> = {};
    for (const row of modelStrategies) {
      if (row.pattern.trim()) ms[row.pattern.trim()] = row.strategy;
    }
    const mf: Record<string, string[]> = {};
    for (const row of modelFallbacks) {
      if (row.pattern.trim()) {
        mf[row.pattern.trim()] = row.fallbacks.split(',').map((s) => s.trim()).filter(Boolean);
      }
    }
    return {
      strategy,
      fallback_enabled: fallbackEnabled,
      request_retry: requestRetry,
      max_retry_interval: maxRetryInterval,
      model_strategies: ms,
      model_fallbacks: mf,
    };
  };

  const handleSave = async () => {
    setSaving(true);
    setError('');
    setSaved(false);

    try {
      const payload = buildPayload();
      await routingApi.update(payload);
      loadConfig({
        strategy: payload.strategy,
        fallback_enabled: payload.fallback_enabled,
        request_retry: payload.request_retry,
        max_retry_interval: payload.max_retry_interval,
        model_strategies: payload.model_strategies,
        model_fallbacks: payload.model_fallbacks,
      });
      setSaved(true);
      setTimeout(() => setSaved(false), 3000);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to update routing config');
    } finally {
      setSaving(false);
    }
  };

  const handleReset = () => {
    if (config) loadConfig(config);
  };

  if (isLoading) {
    return (
      <div className="page">
        <div className="page-header">
          <h2>Routing</h2>
        </div>
        <div className="card">
          <div className="card-body">Loading...</div>
        </div>
      </div>
    );
  }

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h2>Routing</h2>
          <p className="page-subtitle">Configure request routing strategy</p>
        </div>
        <div className="page-header-actions">
          <button className="btn btn-secondary" onClick={handleReset}>
            <RotateCcw size={16} />
            Reset
          </button>
          <button
            className="btn btn-primary"
            onClick={handleSave}
            disabled={saving}
          >
            <Save size={16} />
            {saving ? 'Saving...' : saved ? 'Saved!' : 'Save Changes'}
          </button>
        </div>
      </div>

      {error && <div className="alert alert-error" style={{ marginBottom: '1.5rem' }}>{error}</div>}
      {saved && <div className="alert alert-success" style={{ marginBottom: '1.5rem' }}>Routing configuration updated successfully.</div>}

      {/* Strategy Selection */}
      <div className="card">
        <div className="card-header">
          <h3>Default Routing Strategy</h3>
        </div>
        <div className="card-body">
          <div className="strategy-grid">
            {STRATEGIES.map((s) => (
              <label
                key={s.value}
                className={`strategy-option ${strategy === s.value ? 'strategy-option--selected' : ''}`}
              >
                <input
                  type="radio"
                  name="strategy"
                  value={s.value}
                  checked={strategy === s.value}
                  onChange={() => setStrategy(s.value)}
                />
                <div className="strategy-option-content">
                  <div className="strategy-option-header">
                    <GitBranch size={18} />
                    <span className="strategy-option-label">{s.label}</span>
                  </div>
                  <p className="strategy-option-desc">{s.description}</p>
                </div>
              </label>
            ))}
          </div>
        </div>
      </div>

      {/* Per-Model Strategies */}
      <div className="card">
        <div className="card-header card-header--actions">
          <h3>Per-Model Strategies</h3>
          <button
            className="btn btn-ghost btn-sm"
            onClick={() => setModelStrategies([...modelStrategies, { pattern: '', strategy: 'round-robin' }])}
          >
            <Plus size={14} />
            Add Rule
          </button>
        </div>
        <div className="card-body">
          {modelStrategies.length === 0 ? (
            <p className="text-muted">
              No per-model strategy overrides. All models use the default strategy above.
            </p>
          ) : (
            <div className="kv-rows">
              {modelStrategies.map((row, idx) => (
                <div key={idx} className="kv-row">
                  <input
                    type="text"
                    value={row.pattern}
                    onChange={(e) => {
                      const next = [...modelStrategies];
                      next[idx] = { ...next[idx], pattern: e.target.value };
                      setModelStrategies(next);
                    }}
                    placeholder="claude-*, gpt-4o"
                    style={{ flex: 1 }}
                  />
                  <select
                    value={row.strategy}
                    onChange={(e) => {
                      const next = [...modelStrategies];
                      next[idx] = { ...next[idx], strategy: e.target.value as RoutingStrategy };
                      setModelStrategies(next);
                    }}
                  >
                    {STRATEGIES.map((s) => (
                      <option key={s.value} value={s.value}>{s.label}</option>
                    ))}
                  </select>
                  <button
                    className="btn btn-ghost btn-sm btn-danger-ghost"
                    onClick={() => setModelStrategies(modelStrategies.filter((_, i) => i !== idx))}
                  >
                    <Trash2 size={14} />
                  </button>
                </div>
              ))}
            </div>
          )}
          <span className="form-help">
            Model patterns support glob wildcards (*). More specific patterns take priority over the default.
          </span>
        </div>
      </div>

      {/* Model Fallbacks */}
      <div className="card">
        <div className="card-header card-header--actions">
          <h3>Model Fallbacks</h3>
          <button
            className="btn btn-ghost btn-sm"
            onClick={() => setModelFallbacks([...modelFallbacks, { pattern: '', fallbacks: '' }])}
          >
            <Plus size={14} />
            Add Fallback
          </button>
        </div>
        <div className="card-body">
          {modelFallbacks.length === 0 ? (
            <p className="text-muted">
              No model fallback chains configured. Requests will only try the requested model.
            </p>
          ) : (
            <div className="kv-rows">
              {modelFallbacks.map((row, idx) => (
                <div key={idx} className="kv-row">
                  <input
                    type="text"
                    value={row.pattern}
                    onChange={(e) => {
                      const next = [...modelFallbacks];
                      next[idx] = { ...next[idx], pattern: e.target.value };
                      setModelFallbacks(next);
                    }}
                    placeholder="gpt-4o"
                    style={{ width: '30%' }}
                  />
                  <span className="text-muted" style={{ flexShrink: 0 }}>&rarr;</span>
                  <input
                    type="text"
                    value={row.fallbacks}
                    onChange={(e) => {
                      const next = [...modelFallbacks];
                      next[idx] = { ...next[idx], fallbacks: e.target.value };
                      setModelFallbacks(next);
                    }}
                    placeholder="gpt-4o-mini, gpt-3.5-turbo"
                    style={{ flex: 1 }}
                  />
                  <button
                    className="btn btn-ghost btn-sm btn-danger-ghost"
                    onClick={() => setModelFallbacks(modelFallbacks.filter((_, i) => i !== idx))}
                  >
                    <Trash2 size={14} />
                  </button>
                </div>
              ))}
            </div>
          )}
          <span className="form-help">
            When all credentials for the primary model are exhausted, the system tries fallback models in order. Comma-separated.
          </span>
        </div>
      </div>

      {/* Additional Settings */}
      <div className="card">
        <div className="card-header">
          <h3>Settings</h3>
        </div>
        <div className="card-body">
          <div className="settings-form">
            <div className="form-group form-group-inline">
              <label className="checkbox-label">
                <input
                  type="checkbox"
                  checked={fallbackEnabled}
                  onChange={(e) => setFallbackEnabled(e.target.checked)}
                />
                Enable fallback (model fallback chains and provider retry on failure)
              </label>
            </div>

            <div className="form-row">
              <div className="form-group">
                <label>Retry Count</label>
                <input
                  type="number"
                  value={requestRetry}
                  onChange={(e) => setRequestRetry(parseInt(e.target.value, 10) || 0)}
                  min="0"
                  max="10"
                />
                <span className="form-help">Number of retries on failure (0-10)</span>
              </div>

              <div className="form-group">
                <label>Max Retry Interval (seconds)</label>
                <input
                  type="number"
                  value={maxRetryInterval}
                  onChange={(e) => setMaxRetryInterval(parseInt(e.target.value, 10) || 5)}
                  min="1"
                  max="300"
                />
                <span className="form-help">Maximum wait between retries in seconds</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
