import { useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { KpiCard } from '../components/KpiCard';
import { Panel } from '../components/Panel';
import { StatusPill } from '../components/StatusPill';
import { WorkbenchSheet } from '../components/WorkbenchSheet';
import { WORKSPACES } from '../constants/workspaces';
import { useI18n } from '../i18n';
import { useCommandCenterData } from '../hooks/useWorkspaceData';
import { configApi } from '../services/config';
import { getApiErrorMessage } from '../services/errors';
import { systemApi } from '../services/system';
import { presentFactValue } from '../lib/operatorPresentation';
import type { SystemHealthResponse, SystemLogEntry } from '../types/backend';
import type { WorkspaceId } from '../types/shell';

export function CommandCenterPage() {
  const { t, tx, formatNumber } = useI18n();
  const { data, error, loading } = useCommandCenterData();
  const navigate = useNavigate();
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [diagnosticsOpen, setDiagnosticsOpen] = useState(false);
  const [systemHealth, setSystemHealth] = useState<SystemHealthResponse | null>(null);
  const [recentLogs, setRecentLogs] = useState<SystemLogEntry[]>([]);
  const [systemError, setSystemError] = useState<string | null>(null);
  const [diagnosticLogs, setDiagnosticLogs] = useState<SystemLogEntry[]>([]);
  const [diagnosticsLoading, setDiagnosticsLoading] = useState(false);
  const [diagnosticsError, setDiagnosticsError] = useState<string | null>(null);
  const [diagnosticSearch, setDiagnosticSearch] = useState('');
  const [diagnosticLevel, setDiagnosticLevel] = useState('');
  const [diagnosticTotal, setDiagnosticTotal] = useState(0);
  const [diagnosticFile, setDiagnosticFile] = useState<string | null>(null);
  const [diagnosticTruncated, setDiagnosticTruncated] = useState(false);
  const [repairing, setRepairing] = useState(false);
  const [repairStatus, setRepairStatus] = useState<string | null>(null);

  const workspaceLabel = (workspaceId?: WorkspaceId | null) =>
    tx(
      WORKSPACES.find((workspace) => workspace.id === workspaceId)?.label ??
        WORKSPACES[0].label,
    );

  const firstSignal = data?.signals[0] ?? null;
  const investigationSignal = useMemo(
    () =>
      data?.signals.find((signal) => signal.target_workspace !== 'command-center') ??
      firstSignal,
    [data?.signals, firstSignal],
  );
  const quickActions = useMemo(
    () => [
      { id: 'traffic-lab', label: t('commandCenter.palette.openLiveInvestigation'), path: '/traffic-lab' },
      { id: 'provider-atlas', label: t('commandCenter.palette.inspectProviderRoster'), path: '/provider-atlas' },
      { id: 'route-studio', label: t('commandCenter.palette.reviewRouteDraft'), path: '/route-studio' },
      { id: 'change-studio', label: t('commandCenter.palette.openStructuredChange'), path: '/change-studio' },
    ],
    [t],
  );

  useEffect(() => {
    void (async () => {
      try {
        const [health, logs] = await Promise.all([
          systemApi.health(),
          systemApi.logs({ page: 1, page_size: 3 }),
        ]);
        setSystemHealth(health);
        setRecentLogs(logs.logs);
      } catch (loadError) {
        setSystemError(getApiErrorMessage(loadError, t('commandCenter.error.loadSystemWatch')));
      }
    })();
  }, [t]);

  const loadDiagnostics = async (search = diagnosticSearch, level = diagnosticLevel) => {
    setDiagnosticsLoading(true);
    setDiagnosticsError(null);
    try {
      const response = await systemApi.logs({
        page: 1,
        page_size: 25,
        search: search || undefined,
        level: level || undefined,
      });
      setDiagnosticLogs(response.logs);
      setDiagnosticTotal(response.total);
      setDiagnosticFile(response.file ?? null);
      setDiagnosticTruncated(response.truncated ?? false);
    } catch (loadError) {
      setDiagnosticsError(getApiErrorMessage(loadError, t('commandCenter.error.loadDiagnostics')));
    } finally {
      setDiagnosticsLoading(false);
    }
  };

  const reloadRuntime = async () => {
    setRepairing(true);
    setRepairStatus(null);
    setDiagnosticsError(null);
    try {
      await configApi.reload();
      setRepairStatus(t('commandCenter.diagnostics.reloadComplete'));
      const [health, logs] = await Promise.all([
        systemApi.health(),
        systemApi.logs({ page: 1, page_size: 3 }),
      ]);
      setSystemHealth(health);
      setRecentLogs(logs.logs);
      await loadDiagnostics();
    } catch (reloadError) {
      setDiagnosticsError(getApiErrorMessage(reloadError, t('commandCenter.error.reloadRuntime')));
    } finally {
      setRepairing(false);
    }
  };

  return (
    <div className="workspace-grid workspace-grid--command">
      <section className="hero">
        <div>
          <p className="workspace-eyebrow">{t('commandCenter.hero.eyebrow')}</p>
          <h1>{t('commandCenter.hero.title')}</h1>
          <p className="workspace-summary">{t('commandCenter.hero.summary')}</p>
        </div>
        <div className="hero-actions">
          <button
            className="button button--primary"
            onClick={() => navigate(`/${investigationSignal?.target_workspace ?? 'command-center'}`)}
          >
            {t('commandCenter.hero.openInvestigation')}
          </button>
          <button
            className="button button--ghost"
            onClick={() => {
              setDiagnosticsOpen(true);
              void loadDiagnostics();
            }}
          >
            {t('commandCenter.hero.diagnostics')}
          </button>
          <button className="button button--ghost" onClick={() => setPaletteOpen(true)}>
            {t('commandCenter.hero.commandPalette')}
          </button>
        </div>
      </section>

      <section className="kpi-strip">
        {(data?.kpis ?? []).map((metric) => (
          <KpiCard
            key={`${metric.label.key}-${metric.value}`}
            label={tx(metric.label)}
            value={metric.value}
            delta={tx(metric.delta)}
          />
        ))}
      </section>

      <Panel
        title={t('commandCenter.panel.signalQueue.title')}
        subtitle={t('commandCenter.panel.signalQueue.subtitle')}
        className="panel--wide"
      >
        <div className="signal-list">
          {loading && !data ? <p>{t('commandCenter.loading.runtimeSignals')}</p> : null}
          {error && !data ? <p>{error}</p> : null}
          {(data?.signals ?? []).map((signal) => (
            <article
              key={signal.id}
              className="signal-row signal-row--interactive"
              onClick={() => navigate(`/${signal.target_workspace}`)}
            >
              <div>
                <strong>{tx(signal.title)}</strong>
                <p>{tx(signal.detail)}</p>
              </div>
              <div className="signal-row__meta">
                <StatusPill label={tx(signal.severity)} tone={signal.severity_tone} />
                <span>{workspaceLabel(signal.target_workspace)}</span>
              </div>
            </article>
          ))}
        </div>
      </Panel>

      <div className="two-column">
        <Panel
          title={t('commandCenter.panel.pressureMap.title')}
          subtitle={t('commandCenter.panel.pressureMap.subtitle')}
        >
          <ul className="fact-list">
            {(data?.pressure_map ?? []).map((fact) => (
              <li key={`${fact.label.key}-${fact.value}`}>
                <span>{tx(fact.label)}</span>
                <strong>{presentFactValue(fact, tx)}</strong>
              </li>
            ))}
          </ul>
        </Panel>
        <Panel
          title={t('commandCenter.panel.watchWindows.title')}
          subtitle={t('commandCenter.panel.watchWindows.subtitle')}
        >
          <ul className="fact-list">
            {(data?.watch_windows ?? []).map((fact) => (
              <li key={`${fact.label.key}-${fact.value}`}>
                <span>{tx(fact.label)}</span>
                <strong>{presentFactValue(fact, tx)}</strong>
              </li>
            ))}
          </ul>
        </Panel>
      </div>

      <div className="two-column">
        <Panel
          title={t('commandCenter.panel.systemWatch.title')}
          subtitle={t('commandCenter.panel.systemWatch.subtitle')}
        >
          {systemError ? <div className="status-message status-message--danger">{systemError}</div> : null}
          {systemHealth ? (
            <ul className="fact-list">
              <li><span>{t('common.status')}</span><strong>{systemHealth.status}</strong></li>
              <li><span>{t('common.version')}</span><strong>{systemHealth.version}</strong></li>
              <li><span>{t('commandCenter.systemWatch.uptime')}</span><strong>{formatNumber(systemHealth.uptime_seconds)}s</strong></li>
              <li><span>{t('common.providers')}</span><strong>{formatNumber(systemHealth.providers.length)}</strong></li>
            </ul>
          ) : (
            <div className="status-message">{t('commandCenter.loading.systemPosture')}</div>
          )}
        </Panel>

        <Panel
          title={t('commandCenter.panel.recentLogs.title')}
          subtitle={t('commandCenter.panel.recentLogs.subtitle')}
        >
          <div className="inline-actions">
            <button
              type="button"
              className="button button--ghost"
              onClick={() => {
                setDiagnosticsOpen(true);
                void loadDiagnostics();
              }}
            >
              {t('commandCenter.panel.recentLogs.openDiagnostics')}
            </button>
          </div>
          {recentLogs.length === 0 ? (
            <div className="status-message">{t('commandCenter.panel.recentLogs.empty')}</div>
          ) : (
            <div className="probe-list">
              {recentLogs.map((entry, index) => (
                <div key={`${entry.timestamp}-${index}`} className="probe-check">
                  <span>{entry.level}</span>
                  <strong>{entry.message}</strong>
                </div>
              ))}
            </div>
          )}
        </Panel>
      </div>

      <WorkbenchSheet
        open={paletteOpen}
        onClose={() => setPaletteOpen(false)}
        title={t('commandCenter.palette.title')}
        subtitle={t('commandCenter.palette.subtitle')}
      >
        <section className="sheet-section">
          <h3>{t('commandCenter.palette.quickActions')}</h3>
          <div className="action-stack">
            {quickActions.map((action) => (
              <button
                key={action.id}
                type="button"
                className="button button--secondary button--block"
                onClick={() => {
                  setPaletteOpen(false);
                  navigate(action.path);
                }}
              >
                {action.label}
              </button>
            ))}
          </div>
        </section>

        {firstSignal ? (
          <section className="sheet-section">
            <h3>{t('commandCenter.palette.topLiveSignal')}</h3>
            <div className="detail-grid">
              <div className="detail-grid__row"><span>{t('commandCenter.palette.signalTitle')}</span><strong>{tx(firstSignal.title)}</strong></div>
              <div className="detail-grid__row"><span>{t('commandCenter.palette.signalWorkspace')}</span><strong>{workspaceLabel(firstSignal.target_workspace)}</strong></div>
              <div className="detail-grid__row"><span>{t('commandCenter.palette.signalSeverity')}</span><strong>{tx(firstSignal.severity)}</strong></div>
            </div>
          </section>
        ) : null}
      </WorkbenchSheet>

      <WorkbenchSheet
        open={diagnosticsOpen}
        onClose={() => setDiagnosticsOpen(false)}
        title={t('commandCenter.diagnostics.title')}
        subtitle={t('commandCenter.diagnostics.subtitle')}
        actions={(
          <>
            <button
              type="button"
              className="button button--ghost"
              onClick={() => void loadDiagnostics()}
              disabled={diagnosticsLoading}
            >
              {diagnosticsLoading ? t('common.loading') : t('commandCenter.diagnostics.refreshLogs')}
            </button>
            <button
              type="button"
              className="button button--primary"
              onClick={() => void reloadRuntime()}
              disabled={repairing}
            >
              {repairing ? t('commandCenter.diagnostics.reloading') : t('commandCenter.diagnostics.reloadRuntime')}
            </button>
          </>
        )}
      >
        {repairStatus ? <div className="status-message status-message--success">{repairStatus}</div> : null}
        {diagnosticsError ? <div className="status-message status-message--danger">{diagnosticsError}</div> : null}
        <section className="sheet-section">
          <h3>{t('commandCenter.diagnostics.logSearch')}</h3>
          <div className="sheet-form">
            <label className="sheet-field">
              <span>{t('common.search')}</span>
              <input
                name="diagnostic-log-search"
                autoComplete="off"
                value={diagnosticSearch}
                onChange={(event) => setDiagnosticSearch(event.target.value)}
              />
            </label>
            <label className="sheet-field">
              <span>{t('common.level')}</span>
              <select value={diagnosticLevel} onChange={(event) => setDiagnosticLevel(event.target.value)}>
                <option value="">{t('common.all')}</option>
                <option value="ERROR">ERROR</option>
                <option value="WARN">WARN</option>
                <option value="INFO">INFO</option>
                <option value="DEBUG">DEBUG</option>
              </select>
            </label>
          </div>
          <div className="inline-actions">
            <button
              type="button"
              className="button button--ghost"
              onClick={() => void loadDiagnostics(diagnosticSearch, diagnosticLevel)}
            >
              {t('commandCenter.diagnostics.applyFilters')}
            </button>
          </div>
          <div className="detail-grid">
            <div className="detail-grid__row"><span>{t('commandCenter.diagnostics.totalHits')}</span><strong>{formatNumber(diagnosticTotal)}</strong></div>
            <div className="detail-grid__row"><span>{t('commandCenter.diagnostics.file')}</span><strong>{diagnosticFile ?? t('common.notAvailable')}</strong></div>
            <div className="detail-grid__row"><span>{t('commandCenter.diagnostics.truncatedTail')}</span><strong>{diagnosticTruncated ? t('common.yes') : t('common.no')}</strong></div>
          </div>
        </section>

        <section className="sheet-section">
          <h3>{t('commandCenter.diagnostics.matchingLines')}</h3>
          {diagnosticsLoading ? <div className="status-message">{t('commandCenter.loading.diagnosticsLogs')}</div> : null}
          {diagnosticLogs.length === 0 && !diagnosticsLoading ? (
            <div className="status-message">{t('commandCenter.diagnostics.empty')}</div>
          ) : (
            <div className="probe-list">
              {diagnosticLogs.map((entry, index) => (
                <div key={`${entry.timestamp}-${entry.level}-${index}`} className="probe-check">
                  <span>{entry.level} · {entry.target || t('common.runtime')}</span>
                  <strong>{entry.message}</strong>
                </div>
              ))}
            </div>
          )}
        </section>
      </WorkbenchSheet>
    </div>
  );
}
