import { GitBranch, Shield, Zap, DollarSign } from 'lucide-react';

const PRESETS = [
  {
    key: 'balanced',
    label: 'Balanced',
    intent: 'Distribute requests evenly across providers and credentials',
    algorithm: 'weighted-round-robin',
    icon: GitBranch,
  },
  {
    key: 'stable',
    label: 'Stable',
    intent: 'Always use the same provider, failover only when unhealthy',
    algorithm: 'ordered-fallback',
    icon: Shield,
  },
  {
    key: 'lowest-latency',
    label: 'Lowest Latency',
    intent: 'Route to the fastest responding provider',
    algorithm: 'ewma-latency',
    icon: Zap,
  },
  {
    key: 'lowest-cost',
    label: 'Lowest Cost',
    intent: 'Route to the cheapest available provider',
    algorithm: 'lowest-estimated-cost',
    icon: DollarSign,
  },
];

interface PresetCardsProps {
  selectedProfile: string;
  onSelect: (profile: string) => void;
}

export default function PresetCards({ selectedProfile, onSelect }: PresetCardsProps) {
  return (
    <div className="card">
      <div className="card-header">
        <h3>Routing Mode</h3>
      </div>
      <div className="card-body">
        <div className="strategy-grid">
          {PRESETS.map((p) => {
            const Icon = p.icon;
            return (
              <label
                key={p.key}
                className={`strategy-option ${selectedProfile === p.key ? 'strategy-option--selected' : ''}`}
              >
                <input
                  type="radio"
                  name="profile"
                  value={p.key}
                  checked={selectedProfile === p.key}
                  onChange={() => onSelect(p.key)}
                />
                <div className="strategy-option-content">
                  <div className="strategy-option-header">
                    <Icon size={18} />
                    <span className="strategy-option-label">{p.label}</span>
                  </div>
                  <p className="strategy-option-desc">{p.intent}</p>
                  <p className="strategy-option-algo">{p.algorithm}</p>
                </div>
              </label>
            );
          })}
        </div>
      </div>
    </div>
  );
}
