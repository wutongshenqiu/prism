import { useEffect, useState } from 'react';
import { routingApi } from '../services/api';
import type { RoutingConfig, RouteRule } from '../types';
import { Save, RotateCcw } from 'lucide-react';
import PresetCards from '../components/routing/PresetCards';
import RuleTable from '../components/routing/RuleTable';
import AdvancedEditor from '../components/routing/AdvancedEditor';
import RoutePreview from '../components/routing/RoutePreview';

export default function Routing() {
  const [config, setConfig] = useState<RoutingConfig | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState('');

  const [selectedProfile, setSelectedProfile] = useState('balanced');
  const [rules, setRules] = useState<RouteRule[]>([]);
  const [dirty, setDirty] = useState(false);

  const loadConfig = (data: RoutingConfig) => {
    setConfig(data);
    setSelectedProfile(data['default-profile']);
    setRules(data.rules ?? []);
    setDirty(false);
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

  const handleProfileSelect = (profile: string) => {
    setSelectedProfile(profile);
    setDirty(true);
  };

  const handleRulesChange = (newRules: RouteRule[]) => {
    setRules(newRules);
    setDirty(true);
  };

  const handleSave = async () => {
    setSaving(true);
    setError('');
    setSaved(false);

    try {
      await routingApi.update({
        'default-profile': selectedProfile,
        rules,
      });
      if (config) {
        loadConfig({ ...config, 'default-profile': selectedProfile, rules });
      }
      setSaved(true);
      setTimeout(() => setSaved(false), 3000);
    } catch (err) {
      if (err && typeof err === 'object' && 'response' in err) {
        const axiosErr = err as { response?: { data?: { details?: string[] } } };
        const details = axiosErr.response?.data?.details;
        if (details && details.length > 0) {
          setError(details.join('; '));
        } else {
          setError('Failed to update routing config');
        }
      } else {
        setError(err instanceof Error ? err.message : 'Failed to update routing config');
      }
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

  const profileNames = config ? Object.keys(config.profiles) : [];
  const activeProfile = config?.profiles[selectedProfile];

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h2>Routing</h2>
          <p className="page-subtitle">Configure request routing profile, rules, and preview decisions</p>
        </div>
        <div className="page-header-actions">
          <button className="btn btn-secondary" onClick={handleReset} disabled={!dirty}>
            <RotateCcw size={16} />
            Reset
          </button>
          <button
            className="btn btn-primary"
            onClick={handleSave}
            disabled={saving || !dirty}
          >
            <Save size={16} />
            {saving ? 'Saving...' : saved ? 'Saved!' : 'Save Changes'}
          </button>
        </div>
      </div>

      {error && <div className="alert alert-error" style={{ marginBottom: '1.5rem' }}>{error}</div>}
      {saved && <div className="alert alert-success" style={{ marginBottom: '1.5rem' }}>Routing configuration updated successfully.</div>}

      {/* Section 1: Preset Cards */}
      <PresetCards selectedProfile={selectedProfile} onSelect={handleProfileSelect} />

      {/* Section 2: Rules */}
      <RuleTable rules={rules} profileNames={profileNames} onChange={handleRulesChange} />

      {/* Section 3: Advanced Policy */}
      {activeProfile && (
        <AdvancedEditor profileName={selectedProfile} profile={activeProfile} />
      )}

      {/* Section 4: Route Preview */}
      <RoutePreview />
    </div>
  );
}
