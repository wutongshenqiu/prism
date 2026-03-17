import { Panel } from '../Panel';
import { StatusPill } from '../StatusPill';
import { headersToDraft } from '../../lib/routeStudio';
import type { RouteRule, RoutingConfig } from '../../types/backend';
import type { RouteScenarioRow, RouteStudioResponse } from '../../types/controlPlane';

interface RouteStudioOverviewProps {
  loading: boolean;
  error: string | null;
  data: RouteStudioResponse | null;
  selectedScenario: RouteScenarioRow | null;
  selectedScenarioId: string | null;
  routingLoading: boolean;
  routingDraft: RoutingConfig | null;
  routingConfig: RoutingConfig | null;
  routingStatus: string | null;
  routingError: string | null;
  savingDraft: boolean;
  profileNames: string[];
  selectedProfileName: string | null;
  selectedRuleIndex: number | null;
  selectedRule: RouteRule | null;
  profileJsonDraft: string;
  modelResolutionDraft: string;
  onSelectScenario: (scenarioId: string) => void;
  onDefaultProfileChange: (profileName: string) => void;
  onSelectedProfileChange: (profileName: string) => void;
  onResetDraft: () => void;
  onSaveDraft: () => void;
  onRuleFieldUpdate: (field: keyof RouteRule, value: string) => void;
  onRuleMatchUpdate: (field: keyof RouteRule['match'], value: string | boolean) => void;
  onCreateRule: () => void;
  onDeleteSelectedRule: () => void;
  onSelectRuleIndex: (index: number) => void;
  onProfileJsonChange: (value: string) => void;
  onModelResolutionDraftChange: (value: string) => void;
}

export function RouteStudioOverview({
  loading,
  error,
  data,
  selectedScenario,
  selectedScenarioId,
  routingLoading,
  routingDraft,
  routingConfig,
  routingStatus,
  routingError,
  savingDraft,
  profileNames,
  selectedProfileName,
  selectedRuleIndex,
  selectedRule,
  profileJsonDraft,
  modelResolutionDraft,
  onSelectScenario,
  onDefaultProfileChange,
  onSelectedProfileChange,
  onResetDraft,
  onSaveDraft,
  onRuleFieldUpdate,
  onRuleMatchUpdate,
  onCreateRule,
  onDeleteSelectedRule,
  onSelectRuleIndex,
  onProfileJsonChange,
  onModelResolutionDraftChange,
}: RouteStudioOverviewProps) {
  return (
    <>
      {selectedScenario ? (
        <div className="status-message status-message--warning">
          Active scenario: <strong>{selectedScenario.scenario}</strong> · winner {selectedScenario.winner} · {selectedScenario.delta}
        </div>
      ) : null}
      {routingStatus ? <div className="status-message status-message--success">{routingStatus}</div> : null}
      {routingError ? <div className="status-message status-message--danger">{routingError}</div> : null}

      <div className="two-column">
        <Panel title="Routing summary" subtitle="Current routing truth and selected draft posture.">
          <ul className="fact-list">
            {(data?.summary_facts ?? []).map((fact) => (
              <li key={fact.label}><span>{fact.label}</span><strong>{fact.value}</strong></li>
            ))}
            {routingDraft ? (
              <>
                <li><span>Default profile</span><strong>{routingDraft['default-profile']}</strong></li>
                <li><span>Profiles</span><strong>{profileNames.length}</strong></li>
                <li><span>Rules</span><strong>{routingDraft.rules.length}</strong></li>
              </>
            ) : null}
          </ul>
        </Panel>
        <Panel title="Explain posture" subtitle="Planner behavior distilled into operator-readable facts.">
          <ul className="fact-list">
            {(data?.explain_facts ?? []).map((fact) => (
              <li key={fact.label}><span>{fact.label}</span><strong>{fact.value}</strong></li>
            ))}
          </ul>
        </Panel>
      </div>

      <div className="two-column">
        <Panel title="Routing authoring" subtitle="Default profile switching and profile policy selection stay in the main workbench.">
          {routingLoading && !routingDraft ? <div className="status-message">Loading routing draft…</div> : null}
          <div className="sheet-form">
            <label className="sheet-field">
              <span>Default profile</span>
              <select
                value={routingDraft?.['default-profile'] ?? ''}
                onChange={(event) => onDefaultProfileChange(event.target.value)}
                disabled={!routingDraft}
              >
                {profileNames.map((name) => (
                  <option key={name} value={name}>{name}</option>
                ))}
              </select>
            </label>
            <label className="sheet-field">
              <span>Selected profile</span>
              <select
                value={selectedProfileName ?? ''}
                onChange={(event) => onSelectedProfileChange(event.target.value)}
                disabled={!routingDraft}
              >
                {profileNames.map((name) => (
                  <option key={name} value={name}>{name}</option>
                ))}
              </select>
            </label>
          </div>
          <div className="inline-actions">
            <button type="button" className="button button--ghost" onClick={onResetDraft} disabled={!routingConfig}>
              Reset draft
            </button>
            <button type="button" className="button button--primary" onClick={onSaveDraft} disabled={savingDraft || !routingDraft}>
              {savingDraft ? 'Saving…' : 'Save routing'}
            </button>
          </div>
        </Panel>

        <Panel title="Selected rule editor" subtitle="Rule CRUD belongs here, not in raw YAML.">
          {selectedRule ? (
            <div className="sheet-form">
              <label className="sheet-field">
                <span>Rule name</span>
                <input
                  name="route-rule-name"
                  autoComplete="off"
                  value={selectedRule.name}
                  onChange={(event) => onRuleFieldUpdate('name', event.target.value)}
                />
              </label>
              <label className="sheet-field">
                <span>Priority</span>
                <input
                  name="route-rule-priority"
                  inputMode="numeric"
                  autoComplete="off"
                  value={selectedRule.priority?.toString() ?? ''}
                  onChange={(event) => onRuleFieldUpdate('priority', event.target.value)}
                />
              </label>
              <label className="sheet-field">
                <span>Use profile</span>
                <select
                  value={selectedRule['use-profile']}
                  onChange={(event) => onRuleFieldUpdate('use-profile', event.target.value)}
                >
                  {profileNames.map((name) => (
                    <option key={name} value={name}>{name}</option>
                  ))}
                </select>
              </label>
              <label className="sheet-field">
                <span>Models</span>
                <input
                  name="route-rule-models"
                  autoComplete="off"
                  value={selectedRule.match.models?.join(', ') ?? ''}
                  onChange={(event) => onRuleMatchUpdate('models', event.target.value)}
                />
              </label>
              <label className="sheet-field">
                <span>Tenants</span>
                <input
                  name="route-rule-tenants"
                  autoComplete="off"
                  value={selectedRule.match.tenants?.join(', ') ?? ''}
                  onChange={(event) => onRuleMatchUpdate('tenants', event.target.value)}
                />
              </label>
              <label className="sheet-field">
                <span>Endpoints</span>
                <input
                  name="route-rule-endpoints"
                  autoComplete="off"
                  value={selectedRule.match.endpoints?.join(', ') ?? ''}
                  onChange={(event) => onRuleMatchUpdate('endpoints', event.target.value)}
                />
              </label>
              <label className="sheet-field">
                <span>Regions</span>
                <input
                  name="route-rule-regions"
                  autoComplete="off"
                  value={selectedRule.match.regions?.join(', ') ?? ''}
                  onChange={(event) => onRuleMatchUpdate('regions', event.target.value)}
                />
              </label>
              <label className="sheet-field">
                <span>Headers</span>
                <textarea
                  className="yaml-editor"
                  value={headersToDraft(selectedRule.match.headers)}
                  onChange={(event) => onRuleMatchUpdate('headers', event.target.value)}
                />
              </label>
              <label className="detail-grid__row">
                <span>Streaming only</span>
                <input
                  type="checkbox"
                  checked={selectedRule.match.stream ?? false}
                  onChange={(event) => onRuleMatchUpdate('stream', event.target.checked)}
                />
              </label>
            </div>
          ) : (
            <div className="status-message">Select or create a rule to begin editing.</div>
          )}
        </Panel>
      </div>

      <div className="two-column">
        <Panel title="Rule registry" subtitle="Rules can be added, selected, and removed without losing blast-radius context.">
          <div className="inline-actions">
            <button type="button" className="button button--ghost" onClick={onCreateRule} disabled={!routingDraft}>
              New rule
            </button>
            <button type="button" className="button button--ghost" onClick={onDeleteSelectedRule} disabled={!routingDraft || selectedRuleIndex === null}>
              Delete selected
            </button>
          </div>
          <div className="table-grid table-grid--routes">
            <div className="table-grid__head">Rule</div>
            <div className="table-grid__head">Profile</div>
            <div className="table-grid__head">Priority</div>
            <div className="table-grid__head">Matchers</div>
            {(routingDraft?.rules ?? []).flatMap((rule, index) => {
              const selected = index === selectedRuleIndex;
              const cellClass = `table-grid__cell ${selected ? 'is-selected' : ''} is-clickable`;
              const matchers = [
                rule.match.models?.length ? `${rule.match.models.length} models` : null,
                rule.match.tenants?.length ? `${rule.match.tenants.length} tenants` : null,
                rule.match.endpoints?.length ? `${rule.match.endpoints.length} endpoints` : null,
              ].filter(Boolean).join(' · ') || 'default';
              return [
                <div key={`${rule.name}-name`} className={`${cellClass} table-grid__cell--strong`} onClick={() => onSelectRuleIndex(index)}>
                  {rule.name}
                </div>,
                <div key={`${rule.name}-profile`} className={cellClass} onClick={() => onSelectRuleIndex(index)}>
                  {rule['use-profile']}
                </div>,
                <div key={`${rule.name}-priority`} className={cellClass} onClick={() => onSelectRuleIndex(index)}>
                  {rule.priority ?? 'n/a'}
                </div>,
                <div key={`${rule.name}-match`} className={cellClass} onClick={() => onSelectRuleIndex(index)}>
                  {matchers}
                </div>,
              ];
            })}
          </div>
        </Panel>

        <Panel title="Scenario matrix" subtitle="Sampled route explanations from live traffic and configured models.">
          <div className="table-grid table-grid--routes">
            <div className="table-grid__head">Scenario</div>
            <div className="table-grid__head">Winner</div>
            <div className="table-grid__head">Delta</div>
            <div className="table-grid__head">Route state</div>
            {loading && !data ? <div className="table-grid__cell">Loading scenarios…</div> : null}
            {error && !data ? <div className="table-grid__cell">{error}</div> : null}
            {(data?.scenarios ?? []).flatMap((scenario) => {
              const selected = scenario.scenario === selectedScenarioId;
              const cellClass = `table-grid__cell ${selected ? 'is-selected' : ''} is-clickable`;
              return [
                <div
                  key={`${scenario.scenario}-scenario`}
                  className={`${cellClass} table-grid__cell--strong`}
                  onClick={() => onSelectScenario(scenario.scenario)}
                >
                  {scenario.scenario}
                </div>,
                <div key={`${scenario.scenario}-winner`} className={cellClass} onClick={() => onSelectScenario(scenario.scenario)}>
                  {scenario.winner}
                </div>,
                <div key={`${scenario.scenario}-delta`} className={cellClass} onClick={() => onSelectScenario(scenario.scenario)}>
                  {scenario.delta}
                </div>,
                <div key={`${scenario.scenario}-decision`} className={cellClass} onClick={() => onSelectScenario(scenario.scenario)}>
                  <StatusPill label={scenario.decision} tone={scenario.decision_tone} />
                </div>,
              ];
            })}
          </div>
        </Panel>
      </div>

      <div className="two-column">
        <Panel title="Advanced profile policy" subtitle="Edit the selected route profile directly when the structured fields are not enough.">
          <textarea
            className="yaml-editor"
            value={profileJsonDraft}
            onChange={(event) => onProfileJsonChange(event.target.value)}
            spellCheck={false}
          />
        </Panel>
        <Panel title="Model resolution" subtitle="Alias, rewrite, fallback, and provider pins stay explicit in the same draft.">
          <textarea
            className="yaml-editor"
            value={modelResolutionDraft}
            onChange={(event) => onModelResolutionDraftChange(event.target.value)}
            spellCheck={false}
          />
        </Panel>
      </div>
    </>
  );
}
