import { useEffect, useState } from 'react';
import { routingApi } from '../services/api';
import type { RoutingConfig, RoutingStrategy } from '../types';
import { GitBranch, Save, RotateCcw } from 'lucide-react';

const STRATEGIES: { value: RoutingStrategy; label: string; description: string }[] = [
  {
    value: 'round_robin',
    label: 'Round Robin',
    description: 'Distribute requests evenly across all active providers in sequence.',
  },
  {
    value: 'random',
    label: 'Random',
    description: 'Randomly select a provider for each request.',
  },
  {
    value: 'least_latency',
    label: 'Least Latency',
    description: 'Route to the provider with the lowest average response time.',
  },
  {
    value: 'failover',
    label: 'Failover',
    description: 'Use primary provider; failover to others only on error.',
  },
];

export default function Routing() {
  const [config, setConfig] = useState<RoutingConfig | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState('');

  // Editable state
  const [strategy, setStrategy] = useState<RoutingStrategy>('round_robin');
  const [fallbackEnabled, setFallbackEnabled] = useState(true);
  const [retryCount, setRetryCount] = useState(3);
  const [timeoutMs, setTimeoutMs] = useState(30000);

  const fetchConfig = async () => {
    try {
      const response = await routingApi.get();
      const data = response.data;
      setConfig(data);
      setStrategy(data.strategy);
      setFallbackEnabled(data.fallback_enabled);
      setRetryCount(data.retry_count);
      setTimeoutMs(data.timeout_ms);
    } catch (err) {
      console.error('Failed to fetch routing config:', err);
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    fetchConfig();
  }, []);

  const hasChanges = config && (
    strategy !== config.strategy ||
    fallbackEnabled !== config.fallback_enabled ||
    retryCount !== config.retry_count ||
    timeoutMs !== config.timeout_ms
  );

  const handleSave = async () => {
    setSaving(true);
    setError('');
    setSaved(false);

    try {
      const response = await routingApi.update({
        strategy,
        fallback_enabled: fallbackEnabled,
        retry_count: retryCount,
        timeout_ms: timeoutMs,
      });
      setConfig(response.data);
      setSaved(true);
      setTimeout(() => setSaved(false), 3000);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to update routing config');
    } finally {
      setSaving(false);
    }
  };

  const handleReset = () => {
    if (config) {
      setStrategy(config.strategy);
      setFallbackEnabled(config.fallback_enabled);
      setRetryCount(config.retry_count);
      setTimeoutMs(config.timeout_ms);
    }
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
          {hasChanges && (
            <button className="btn btn-secondary" onClick={handleReset}>
              <RotateCcw size={16} />
              Reset
            </button>
          )}
          <button
            className="btn btn-primary"
            onClick={handleSave}
            disabled={saving || !hasChanges}
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
          <h3>Routing Strategy</h3>
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

      {/* Additional Settings */}
      <div className="card" style={{ marginTop: '1.5rem' }}>
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
                Enable fallback to other providers on failure
              </label>
            </div>

            <div className="form-row">
              <div className="form-group">
                <label>Retry Count</label>
                <input
                  type="number"
                  value={retryCount}
                  onChange={(e) => setRetryCount(parseInt(e.target.value, 10) || 0)}
                  min="0"
                  max="10"
                />
                <span className="form-help">Number of retries on failure (0-10)</span>
              </div>

              <div className="form-group">
                <label>Timeout (ms)</label>
                <input
                  type="number"
                  value={timeoutMs}
                  onChange={(e) => setTimeoutMs(parseInt(e.target.value, 10) || 5000)}
                  min="1000"
                  max="300000"
                  step="1000"
                />
                <span className="form-help">Request timeout in milliseconds</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
