import { useState } from 'react';
import type { RouteProfile } from '../../types';

interface AdvancedEditorProps {
  profileName: string;
  profile: RouteProfile;
}

type Tab = 'provider' | 'credential' | 'health' | 'failover';

const TAB_LABELS: Record<Tab, string> = {
  provider: 'Provider Policy',
  credential: 'Credential Policy',
  health: 'Health',
  failover: 'Failover',
};

export default function AdvancedEditor({ profileName, profile }: AdvancedEditorProps) {
  const [activeTab, setActiveTab] = useState<Tab>('provider');

  return (
    <div className="card">
      <div className="card-header">
        <h3>Advanced Policy: {profileName}</h3>
      </div>
      <div className="card-body">
        <div className="tabs">
          {(Object.keys(TAB_LABELS) as Tab[]).map((tab) => (
            <button
              key={tab}
              className={`tab ${activeTab === tab ? 'tab--active' : ''}`}
              onClick={() => setActiveTab(tab)}
            >
              {TAB_LABELS[tab]}
            </button>
          ))}
        </div>

        <div className="tab-content" style={{ marginTop: '1rem' }}>
          {activeTab === 'provider' && (
            <div className="settings-form">
              <div className="form-row">
                <div className="form-group">
                  <label>Strategy</label>
                  <code>{profile['provider-policy'].strategy}</code>
                </div>
              </div>
              {profile['provider-policy'].weights && Object.keys(profile['provider-policy'].weights).length > 0 && (
                <div className="form-row">
                  <div className="form-group">
                    <label>Weights</label>
                    <code>
                      {Object.entries(profile['provider-policy'].weights)
                        .map(([k, v]) => `${k}=${v}`)
                        .join(', ')}
                    </code>
                  </div>
                </div>
              )}
              {profile['provider-policy'].order && profile['provider-policy'].order.length > 0 && (
                <div className="form-row">
                  <div className="form-group">
                    <label>Order</label>
                    <code>{profile['provider-policy'].order.join(' → ')}</code>
                  </div>
                </div>
              )}
            </div>
          )}

          {activeTab === 'credential' && (
            <div className="settings-form">
              <div className="form-row">
                <div className="form-group">
                  <label>Strategy</label>
                  <code>{profile['credential-policy'].strategy}</code>
                </div>
              </div>
            </div>
          )}

          {activeTab === 'health' && (
            <div className="settings-form">
              <div className="form-row">
                <div className="form-group">
                  <label>Circuit Breaker</label>
                  <code>{profile.health['circuit-breaker'].enabled ? 'Enabled' : 'Disabled'}</code>
                </div>
                <div className="form-group">
                  <label>Failure Threshold</label>
                  <code>{profile.health['circuit-breaker']['failure-threshold']}</code>
                </div>
                <div className="form-group">
                  <label>Cooldown</label>
                  <code>{profile.health['circuit-breaker']['cooldown-seconds']}s</code>
                </div>
              </div>
              <div className="form-row">
                <div className="form-group">
                  <label>Consecutive 5xx</label>
                  <code>{profile.health['outlier-detection']['consecutive-5xx']}</code>
                </div>
                <div className="form-group">
                  <label>Base Eject</label>
                  <code>{profile.health['outlier-detection']['base-eject-seconds']}s</code>
                </div>
                <div className="form-group">
                  <label>Max Eject</label>
                  <code>{profile.health['outlier-detection']['max-eject-seconds']}s</code>
                </div>
              </div>
            </div>
          )}

          {activeTab === 'failover' && (
            <div className="settings-form">
              <div className="form-row">
                <div className="form-group">
                  <label>Credential Attempts</label>
                  <code>{profile.failover['credential-attempts']}</code>
                </div>
                <div className="form-group">
                  <label>Provider Attempts</label>
                  <code>{profile.failover['provider-attempts']}</code>
                </div>
                <div className="form-group">
                  <label>Model Attempts</label>
                  <code>{profile.failover['model-attempts']}</code>
                </div>
              </div>
              <div className="form-row">
                <div className="form-group">
                  <label>Retry Budget Ratio</label>
                  <code>{profile.failover['retry-budget'].ratio}</code>
                </div>
                <div className="form-group">
                  <label>Retry On</label>
                  <code>{profile.failover['retry-on'].join(', ')}</code>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
