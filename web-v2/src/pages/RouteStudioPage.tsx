import { useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { RouteSimulationSheet } from '../components/route-studio/RouteSimulationSheet';
import { RouteStudioOverview } from '../components/route-studio/RouteStudioOverview';
import { useRouteStudioData } from '../hooks/useWorkspaceData';
import {
  buildNewRule,
  cloneRoutingConfig,
  extractValidationMessage,
  parseCsv,
  parseHeaders,
  parseModelResolutionJson,
  parseProfileJson,
  prettyJson,
} from '../lib/routeStudio';
import { writeRouteDraft } from '../lib/routeDraft';
import { getApiErrorMessage } from '../services/errors';
import { routingApi } from '../services/routing';
import type { RouteExplanation, RouteRule, RoutingConfig } from '../types/backend';

export function RouteStudioPage() {
  const { data, error, loading } = useRouteStudioData();
  const navigate = useNavigate();
  const [selectedScenarioId, setSelectedScenarioId] = useState<string | null>(null);
  const [sheetOpen, setSheetOpen] = useState(false);
  const [simulationLoading, setSimulationLoading] = useState(false);
  const [simulationError, setSimulationError] = useState<string | null>(null);
  const [simulationStatus, setSimulationStatus] = useState<string | null>(null);
  const [explanation, setExplanation] = useState<RouteExplanation | null>(null);
  const [routingConfig, setRoutingConfig] = useState<RoutingConfig | null>(null);
  const [routingDraft, setRoutingDraft] = useState<RoutingConfig | null>(null);
  const [routingLoading, setRoutingLoading] = useState(false);
  const [routingError, setRoutingError] = useState<string | null>(null);
  const [routingStatus, setRoutingStatus] = useState<string | null>(null);
  const [savingDraft, setSavingDraft] = useState(false);
  const [selectedProfileName, setSelectedProfileName] = useState<string | null>(null);
  const [selectedRuleIndex, setSelectedRuleIndex] = useState<number | null>(null);
  const [profileJsonDraft, setProfileJsonDraft] = useState('');
  const [modelResolutionDraft, setModelResolutionDraft] = useState('');

  useEffect(() => {
    setSelectedScenarioId((current) => current ?? data?.scenarios[0]?.scenario ?? null);
  }, [data]);

  useEffect(() => {
    let cancelled = false;

    void (async () => {
      setRoutingLoading(true);
      setRoutingError(null);
      try {
        const config = await routingApi.get();
        if (cancelled) {
          return;
        }
        setRoutingConfig(config);
        setRoutingDraft(cloneRoutingConfig(config));
      } catch (loadError) {
        if (cancelled) {
          return;
        }
        setRoutingError(getApiErrorMessage(loadError, 'Failed to load routing config'));
      } finally {
        if (!cancelled) {
          setRoutingLoading(false);
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, []);

  const selectedScenario = useMemo(
    () => data?.scenarios.find((scenario) => scenario.scenario === selectedScenarioId) ?? null,
    [data, selectedScenarioId],
  );
  const profileNames = useMemo(
    () => Object.keys(routingDraft?.profiles ?? {}),
    [routingDraft],
  );
  const selectedProfile = useMemo(
    () => (selectedProfileName && routingDraft ? routingDraft.profiles[selectedProfileName] ?? null : null),
    [routingDraft, selectedProfileName],
  );
  const selectedRule = useMemo(
    () => (routingDraft && selectedRuleIndex !== null ? routingDraft.rules[selectedRuleIndex] ?? null : null),
    [routingDraft, selectedRuleIndex],
  );

  useEffect(() => {
    if (!routingDraft) {
      return;
    }
    setSelectedProfileName((current) => {
      if (current && routingDraft.profiles[current]) {
        return current;
      }
      return routingDraft['default-profile'];
    });
    setSelectedRuleIndex((current) => {
      if (routingDraft.rules.length === 0) {
        return null;
      }
      if (current !== null && current < routingDraft.rules.length) {
        return current;
      }
      return 0;
    });
  }, [routingDraft]);

  useEffect(() => {
    setProfileJsonDraft(selectedProfile ? prettyJson(selectedProfile) : '');
  }, [selectedProfile]);

  useEffect(() => {
    setModelResolutionDraft(routingDraft ? prettyJson(routingDraft['model-resolution']) : '');
  }, [routingDraft]);

  const applyDraftMutation = (mutate: (draft: RoutingConfig) => void) => {
    setRoutingDraft((current) => {
      if (!current) {
        return current;
      }
      const next = cloneRoutingConfig(current);
      mutate(next);
      return next;
    });
    setRoutingStatus(null);
    setRoutingError(null);
  };

  const handleRuleFieldUpdate = (field: keyof RouteRule, value: string) => {
    if (selectedRuleIndex === null) {
      return;
    }
    applyDraftMutation((draft) => {
      const rule = draft.rules[selectedRuleIndex];
      if (!rule) {
        return;
      }
      if (field === 'priority') {
        rule.priority = value ? Number(value) : undefined;
        return;
      }
      if (field === 'name' || field === 'use-profile') {
        rule[field] = value;
      }
    });
  };

  const handleRuleMatchUpdate = (
    field: keyof RouteRule['match'],
    value: string | boolean,
  ) => {
    if (selectedRuleIndex === null) {
      return;
    }
    applyDraftMutation((draft) => {
      const rule = draft.rules[selectedRuleIndex];
      if (!rule) {
        return;
      }
      if (field === 'stream') {
        rule.match.stream = value ? true : undefined;
        return;
      }
      if (field === 'headers') {
        const parsed = parseHeaders(String(value));
        rule.match.headers = parsed;
        return;
      }
      rule.match[field] = parseCsv(String(value));
    });
  };

  const applyAdvancedDrafts = () => {
    if (!routingDraft) {
      return null;
    }

    const next = cloneRoutingConfig(routingDraft);
    if (selectedProfileName) {
      next.profiles[selectedProfileName] = parseProfileJson(profileJsonDraft);
    }
    next['model-resolution'] = parseModelResolutionJson(modelResolutionDraft);
    return next;
  };

  const simulateDraft = async () => {
    if (!selectedScenario) {
      setSimulationError('Select a scenario first.');
      setSheetOpen(true);
      return;
    }

    setSheetOpen(true);
    setSimulationLoading(true);
    setSimulationError(null);
    setSimulationStatus(null);

    try {
      const nextDraft = applyAdvancedDrafts();
      const routeExplanation = await routingApi.explain({
        model: selectedScenario.model,
        endpoint: selectedScenario.endpoint,
        source_format: selectedScenario.source_format,
        tenant_id: selectedScenario.tenant_id,
        api_key_id: selectedScenario.api_key_id,
        region: selectedScenario.region,
        stream: selectedScenario.stream,
        routing_override: nextDraft ?? undefined,
      });
      if (nextDraft) {
        setRoutingDraft(nextDraft);
      }
      setExplanation(routeExplanation);
      setSimulationStatus(`Simulated route for ${selectedScenario.scenario}.`);
    } catch (actionError) {
      setSimulationError(extractValidationMessage(actionError, 'Route simulation failed'));
    } finally {
      setSimulationLoading(false);
    }
  };

  const saveRoutingDraft = async () => {
    if (!routingDraft) {
      return;
    }

    setSavingDraft(true);
    setRoutingError(null);
    setRoutingStatus(null);
    try {
      const nextDraft = applyAdvancedDrafts();
      if (!nextDraft) {
        throw new Error('Routing draft is not ready yet.');
      }
      const result = await routingApi.update({
        'default-profile': nextDraft['default-profile'],
        profiles: nextDraft.profiles,
        rules: nextDraft.rules,
        'model-resolution': nextDraft['model-resolution'],
      });
      setRoutingConfig(cloneRoutingConfig(nextDraft));
      setRoutingDraft(cloneRoutingConfig(nextDraft));
      setRoutingStatus(result.message ?? 'Routing draft saved.');
    } catch (saveError) {
      setRoutingError(extractValidationMessage(saveError, 'Failed to save routing draft'));
    } finally {
      setSavingDraft(false);
    }
  };

  const resetRoutingDraft = () => {
    if (!routingConfig) {
      return;
    }
    const reset = cloneRoutingConfig(routingConfig);
    setRoutingDraft(reset);
    setRoutingStatus('Reset route draft to runtime truth.');
    setRoutingError(null);
  };

  const promoteToChange = () => {
    if (!selectedScenario) {
      setSimulationError('Select a scenario before promoting it.');
      return;
    }

    writeRouteDraft({
      createdAt: new Date().toISOString(),
      scenario: selectedScenario,
      explanation,
    });
    navigate('/change-studio');
  };

  return (
    <div className="workspace-grid">
      <section className="hero">
        <div>
          <p className="workspace-eyebrow">PRISM / ROUTE STUDIO</p>
          <h1>Draft routing truth before publish</h1>
          <p className="workspace-summary">
            Route Studio owns the full authoring loop: default-profile selection, rule mutation, advanced policy editing, and draft simulation before promotion.
          </p>
        </div>
        <div className="hero-actions">
          <button className="button button--primary" onClick={() => void simulateDraft()}>
            Simulate draft
          </button>
          <button className="button button--ghost" onClick={() => void saveRoutingDraft()} disabled={savingDraft || routingLoading}>
            {savingDraft ? 'Saving…' : 'Save routing draft'}
          </button>
          <button className="button button--ghost" onClick={promoteToChange}>
            Promote to change
          </button>
        </div>
      </section>

      <RouteStudioOverview
        loading={loading}
        error={error}
        data={data}
        selectedScenario={selectedScenario}
        selectedScenarioId={selectedScenarioId}
        routingLoading={routingLoading}
        routingDraft={routingDraft}
        routingConfig={routingConfig}
        routingStatus={routingStatus}
        routingError={routingError}
        savingDraft={savingDraft}
        profileNames={profileNames}
        selectedProfileName={selectedProfileName}
        selectedRuleIndex={selectedRuleIndex}
        selectedRule={selectedRule}
        profileJsonDraft={profileJsonDraft}
        modelResolutionDraft={modelResolutionDraft}
        onSelectScenario={setSelectedScenarioId}
        onDefaultProfileChange={(value) => {
          applyDraftMutation((draft) => {
            draft['default-profile'] = value;
          });
          setSelectedProfileName(value);
        }}
        onSelectedProfileChange={setSelectedProfileName}
        onResetDraft={resetRoutingDraft}
        onSaveDraft={() => void saveRoutingDraft()}
        onRuleFieldUpdate={handleRuleFieldUpdate}
        onRuleMatchUpdate={handleRuleMatchUpdate}
        onCreateRule={() => {
          if (!routingDraft) {
            return;
          }
          const fallbackProfile = selectedProfileName ?? routingDraft['default-profile'];
          applyDraftMutation((draft) => {
            draft.rules.push(buildNewRule(fallbackProfile, draft.rules.length));
          });
          setSelectedRuleIndex(routingDraft.rules.length);
        }}
        onDeleteSelectedRule={() => {
          if (!routingDraft || selectedRuleIndex === null) {
            return;
          }
          applyDraftMutation((draft) => {
            draft.rules.splice(selectedRuleIndex, 1);
          });
          setSelectedRuleIndex((current) => {
            if (current === null) return null;
            if (routingDraft.rules.length <= 1) return null;
            return Math.max(0, current - 1);
          });
        }}
        onSelectRuleIndex={setSelectedRuleIndex}
        onProfileJsonChange={setProfileJsonDraft}
        onModelResolutionDraftChange={setModelResolutionDraft}
      />

      <RouteSimulationSheet
        open={sheetOpen}
        simulationLoading={simulationLoading}
        simulationStatus={simulationStatus}
        simulationError={simulationError}
        selectedScenario={selectedScenario}
        explanation={explanation}
        onClose={() => setSheetOpen(false)}
        onPromoteToChange={promoteToChange}
        onSimulateDraft={() => void simulateDraft()}
      />
    </div>
  );
}
