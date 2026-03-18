import { Panel } from '../Panel';
import { StatusPill } from '../StatusPill';
import { useI18n } from '../../i18n';
import { presentFactValue } from '../../lib/operatorPresentation';
import { headersToDraft } from '../../lib/routeStudio';
import type { RouteRule, RoutingConfig } from '../../types/backend';
import type { RouteScenarioRow, RouteStudioResponse } from '../../types/controlPlane';

interface RouteStudioOverviewProps {
  loading: boolean;
  error: string | null;
  data: RouteStudioResponse | null;
  selectedScenario: RouteScenarioRow | null;
  selectedScenarioIndex: number | null;
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
  onSelectScenario: (scenarioIndex: number) => void;
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
  selectedScenarioIndex,
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
  const { t, tx } = useI18n();

  return (
    <>
      {selectedScenario ? (
        <div className="status-message status-message--info">
          {t('routeStudio.status.activeScenario')} <strong>{selectedScenario.scenario}</strong> · {t('routeStudio.status.winner')} {selectedScenario.winner} · {selectedScenario.delta}
        </div>
      ) : null}
      {routingStatus ? <div className="status-message status-message--success">{routingStatus}</div> : null}
      {routingError ? <div className="status-message status-message--danger">{routingError}</div> : null}

      <div className="two-column">
        <Panel title={t('routeStudio.panel.summary.title')} subtitle={t('routeStudio.panel.summary.subtitle')}>
          <ul className="fact-list">
            {(data?.summary_facts ?? []).map((fact) => (
              <li key={`${fact.label.key}-${fact.value}`}><span>{tx(fact.label)}</span><strong>{presentFactValue(fact, tx)}</strong></li>
            ))}
            {routingDraft ? (
              <>
                <li><span>{t('routeStudio.fact.defaultProfile')}</span><strong>{routingDraft['default-profile']}</strong></li>
                <li><span>{t('routeStudio.fact.profiles')}</span><strong>{profileNames.length}</strong></li>
                <li><span>{t('routeStudio.fact.rules')}</span><strong>{routingDraft.rules.length}</strong></li>
              </>
            ) : null}
          </ul>
        </Panel>
        <Panel title={t('routeStudio.panel.explain.title')} subtitle={t('routeStudio.panel.explain.subtitle')}>
          <ul className="fact-list">
            {(data?.explain_facts ?? []).map((fact) => (
              <li key={`${fact.label.key}-${fact.value}`}><span>{tx(fact.label)}</span><strong>{presentFactValue(fact, tx)}</strong></li>
            ))}
          </ul>
        </Panel>
      </div>

      <div className="two-column">
        <Panel title={t('routeStudio.panel.authoring.title')} subtitle={t('routeStudio.panel.authoring.subtitle')}>
          {routingLoading && !routingDraft ? <div className="status-message">{t('routeStudio.loading.draft')}</div> : null}
          <div className="sheet-form">
            <label className="sheet-field">
              <span>{t('routeStudio.fact.defaultProfile')}</span>
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
              <span>{t('routeStudio.authoring.selectedProfile')}</span>
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
              {t('routeStudio.authoring.resetDraft')}
            </button>
            <button type="button" className="button button--primary" onClick={onSaveDraft} disabled={savingDraft || !routingDraft}>
              {savingDraft ? t('routeStudio.authoring.saving') : t('routeStudio.authoring.saveDraft')}
            </button>
          </div>
        </Panel>

        <Panel title={t('routeStudio.panel.ruleEditor.title')} subtitle={t('routeStudio.panel.ruleEditor.subtitle')}>
          {selectedRule ? (
            <div className="sheet-form">
              <label className="sheet-field">
                <span>{t('routeStudio.rule.name')}</span>
                <input name="route-rule-name" autoComplete="off" value={selectedRule.name} onChange={(event) => onRuleFieldUpdate('name', event.target.value)} />
              </label>
              <label className="sheet-field">
                <span>{t('routeStudio.rule.priority')}</span>
                <input name="route-rule-priority" inputMode="numeric" autoComplete="off" value={selectedRule.priority?.toString() ?? ''} onChange={(event) => onRuleFieldUpdate('priority', event.target.value)} />
              </label>
              <label className="sheet-field">
                <span>{t('routeStudio.rule.useProfile')}</span>
                <select value={selectedRule['use-profile']} onChange={(event) => onRuleFieldUpdate('use-profile', event.target.value)}>
                  {profileNames.map((name) => (
                    <option key={name} value={name}>{name}</option>
                  ))}
                </select>
              </label>
              <label className="sheet-field">
                <span>{t('routeStudio.rule.models')}</span>
                <input name="route-rule-models" autoComplete="off" value={selectedRule.match.models?.join(', ') ?? ''} onChange={(event) => onRuleMatchUpdate('models', event.target.value)} />
              </label>
              <label className="sheet-field">
                <span>{t('routeStudio.rule.tenants')}</span>
                <input name="route-rule-tenants" autoComplete="off" value={selectedRule.match.tenants?.join(', ') ?? ''} onChange={(event) => onRuleMatchUpdate('tenants', event.target.value)} />
              </label>
              <label className="sheet-field">
                <span>{t('routeStudio.rule.endpoints')}</span>
                <input name="route-rule-endpoints" autoComplete="off" value={selectedRule.match.endpoints?.join(', ') ?? ''} onChange={(event) => onRuleMatchUpdate('endpoints', event.target.value)} />
              </label>
              <label className="sheet-field">
                <span>{t('routeStudio.rule.regions')}</span>
                <input name="route-rule-regions" autoComplete="off" value={selectedRule.match.regions?.join(', ') ?? ''} onChange={(event) => onRuleMatchUpdate('regions', event.target.value)} />
              </label>
              <label className="sheet-field">
                <span>{t('routeStudio.rule.headers')}</span>
                <textarea className="yaml-editor" value={headersToDraft(selectedRule.match.headers)} onChange={(event) => onRuleMatchUpdate('headers', event.target.value)} />
              </label>
              <label className="detail-grid__row">
                <span>{t('routeStudio.rule.streamOnly')}</span>
                <input type="checkbox" checked={selectedRule.match.stream ?? false} onChange={(event) => onRuleMatchUpdate('stream', event.target.checked)} />
              </label>
            </div>
          ) : (
            <div className="status-message">{t('routeStudio.rule.empty')}</div>
          )}
        </Panel>
      </div>

      <div className="two-column">
        <Panel title={t('routeStudio.panel.registry.title')} subtitle={t('routeStudio.panel.registry.subtitle')}>
          <div className="inline-actions">
            <button type="button" className="button button--ghost" onClick={onCreateRule} disabled={!routingDraft}>
              {t('routeStudio.registry.newRule')}
            </button>
            <button type="button" className="button button--ghost" onClick={onDeleteSelectedRule} disabled={!routingDraft || selectedRuleIndex === null}>
              {t('routeStudio.registry.deleteSelected')}
            </button>
          </div>
          <div className="table-grid table-grid--routes">
            <div className="table-grid__head">{t('routeStudio.registry.rule')}</div>
            <div className="table-grid__head">{t('routeStudio.registry.profile')}</div>
            <div className="table-grid__head">{t('routeStudio.registry.priority')}</div>
            <div className="table-grid__head">{t('routeStudio.registry.matchers')}</div>
            {(routingDraft?.rules ?? []).flatMap((rule, index) => {
              const selected = index === selectedRuleIndex;
              const cellClass = `table-grid__cell ${selected ? 'is-selected' : ''} is-clickable`;
              const matchers = [
                rule.match.models?.length ? t('routeStudio.registry.matchersModels', { count: rule.match.models.length }) : null,
                rule.match.tenants?.length ? t('routeStudio.registry.matchersTenants', { count: rule.match.tenants.length }) : null,
                rule.match.endpoints?.length ? t('routeStudio.registry.matchersEndpoints', { count: rule.match.endpoints.length }) : null,
              ].filter(Boolean).join(' · ') || t('common.default');
              return [
                <div key={`${rule.name}-name`} className={`${cellClass} table-grid__cell--strong`} onClick={() => onSelectRuleIndex(index)}>
                  {rule.name}
                </div>,
                <div key={`${rule.name}-profile`} className={cellClass} onClick={() => onSelectRuleIndex(index)}>
                  {rule['use-profile']}
                </div>,
                <div key={`${rule.name}-priority`} className={cellClass} onClick={() => onSelectRuleIndex(index)}>
                  {rule.priority ?? t('common.notAvailable')}
                </div>,
                <div key={`${rule.name}-match`} className={cellClass} onClick={() => onSelectRuleIndex(index)}>
                  {matchers}
                </div>,
              ];
            })}
          </div>
        </Panel>

        <Panel title={t('routeStudio.panel.scenarios.title')} subtitle={t('routeStudio.panel.scenarios.subtitle')}>
          <div className="table-grid table-grid--routes">
            <div className="table-grid__head">{t('routeStudio.scenario.scenario')}</div>
            <div className="table-grid__head">{t('routeStudio.scenario.winner')}</div>
            <div className="table-grid__head">{t('routeStudio.scenario.delta')}</div>
            <div className="table-grid__head">{t('routeStudio.scenario.routeState')}</div>
            {loading && !data ? <div className="table-grid__cell">{t('routeStudio.loading.scenarios')}</div> : null}
            {error && !data ? <div className="table-grid__cell">{error}</div> : null}
            {(data?.scenarios ?? []).flatMap((scenario, index) => {
              const selected = index === selectedScenarioIndex;
              const cellClass = `table-grid__cell ${selected ? 'is-selected' : ''} is-clickable`;
              return [
                <div key={`${scenario.scenario}-${scenario.model}-${index}-scenario`} className={`${cellClass} table-grid__cell--strong`} onClick={() => onSelectScenario(index)}>
                  {scenario.scenario}
                </div>,
                <div key={`${scenario.scenario}-${scenario.winner}-${index}-winner`} className={cellClass} onClick={() => onSelectScenario(index)}>
                  {scenario.winner}
                </div>,
                <div key={`${scenario.scenario}-${scenario.delta}-${index}-delta`} className={cellClass} onClick={() => onSelectScenario(index)}>
                  {scenario.delta}
                </div>,
                <div key={`${scenario.scenario}-${scenario.decision}-${index}-decision`} className={cellClass} onClick={() => onSelectScenario(index)}>
                  <StatusPill label={tx(scenario.decision)} tone={scenario.decision_tone} />
                </div>,
              ];
            })}
          </div>
        </Panel>
      </div>

      <div className="two-column">
        <Panel title={t('routeStudio.panel.advancedProfile.title')} subtitle={t('routeStudio.panel.advancedProfile.subtitle')}>
          <textarea className="yaml-editor" value={profileJsonDraft} onChange={(event) => onProfileJsonChange(event.target.value)} spellCheck={false} />
        </Panel>
        <Panel title={t('routeStudio.panel.modelResolution.title')} subtitle={t('routeStudio.panel.modelResolution.subtitle')}>
          <textarea className="yaml-editor" value={modelResolutionDraft} onChange={(event) => onModelResolutionDraftChange(event.target.value)} spellCheck={false} />
        </Panel>
      </div>
    </>
  );
}
