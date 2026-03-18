import { useCallback, useEffect, useMemo, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { Panel } from '../components/Panel';
import { PayloadViewer } from '../components/PayloadViewer';
import { StatusPill } from '../components/StatusPill';
import { WorkbenchSheet } from '../components/WorkbenchSheet';
import { useI18n } from '../i18n';
import { useTrafficLabData } from '../hooks/useWorkspaceData';
import { presentFactValue } from '../lib/operatorPresentation';
import { getApiErrorMessage } from '../services/errors';
import { logsApi } from '../services/logs';
import { routingApi } from '../services/routing';
import { useShellStore } from '../stores/shellStore';
import type { RequestLog, RouteExplanation } from '../types/backend';

function endpointFromPath(path: string) {
  if (path.includes('/messages')) return 'messages';
  if (path.includes('/responses')) return 'responses';
  if (path.includes('streamGenerateContent')) return 'stream-generate-content';
  if (path.includes('generateContent')) return 'generate-content';
  return 'chat-completions';
}

function sourceFormatFromPath(path: string) {
  if (path.includes('/messages')) return 'claude';
  if (path.includes('generateContent')) return 'gemini';
  return 'openai';
}

export function TrafficLabPage() {
  const { t, tx, formatDurationMs } = useI18n();
  const { data, error, loading } = useTrafficLabData();
  const timeRange = useShellStore((state) => state.timeRange);
  const sourceMode = useShellStore((state) => state.sourceMode);
  const live = useShellStore((state) => state.live);
  const [searchParams, setSearchParams] = useSearchParams();
  const [lensStatus, setLensStatus] = useState<string | null>(null);
  const [replayOpen, setReplayOpen] = useState(false);
  const [replayLoading, setReplayLoading] = useState(false);
  const [replayError, setReplayError] = useState<string | null>(null);
  const [requestRecord, setRequestRecord] = useState<RequestLog | null>(null);
  const [explanation, setExplanation] = useState<RouteExplanation | null>(null);
  const [detailOpen, setDetailOpen] = useState(false);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [detailRecord, setDetailRecord] = useState<RequestLog | null>(null);
  const selectedRequestId = searchParams.get('request');
  const compareRequestId = searchParams.get('compare');
  const sessionFilter = searchParams.get('q') ?? '';

  const updateSearch = useCallback((patch: Record<string, string | null | undefined>) => {
    const next = new URLSearchParams(searchParams);
    Object.entries(patch).forEach(([key, value]) => {
      if (!value) {
        next.delete(key);
      } else {
        next.set(key, value);
      }
    });
    setSearchParams(next, { replace: true });
  }, [searchParams, setSearchParams]);

  useEffect(() => {
    const fallback = data?.selected_request_id ?? data?.sessions[0]?.request_id ?? null;
    if (!selectedRequestId && fallback) {
      updateSearch({ request: fallback });
    }
  }, [data, selectedRequestId, updateSearch]);

  const visibleSessions = useMemo(() => {
    const needle = sessionFilter.trim().toLowerCase();
    if (!needle) {
      return data?.sessions ?? [];
    }
    return (data?.sessions ?? []).filter((session) => {
      const haystack = [
        session.request_id,
        session.model,
        tx(session.decision),
        tx(session.result),
      ].join(' ').toLowerCase();
      return haystack.includes(needle);
    });
  }, [data?.sessions, sessionFilter, tx]);

  const selectedSession = useMemo(
    () => data?.sessions.find((session) => session.request_id === selectedRequestId) ?? null,
    [data, selectedRequestId],
  );
  const compareSession = useMemo(
    () => data?.sessions.find((session) => session.request_id === compareRequestId) ?? null,
    [compareRequestId, data],
  );

  const handleSaveLens = () => {
    const payload = {
      timeRange,
      sourceMode,
      selectedRequestId,
      compareRequestId,
      sessionFilter,
      savedAt: new Date().toISOString(),
    };
    localStorage.setItem('prism-control-plane:traffic-lens', JSON.stringify(payload));
    setLensStatus(t('trafficLab.status.lensSaved'));
  };

  const handleReplay = async () => {
    if (!selectedRequestId) {
      setReplayError(t('trafficLab.error.selectSession'));
      setReplayOpen(true);
      return;
    }

    setReplayOpen(true);
    setReplayLoading(true);
    setReplayError(null);

    try {
      const record = await logsApi.getRequest(selectedRequestId);
      if (!record) {
        throw new Error(t('trafficLab.error.requestMissing'));
      }
      setRequestRecord(record);
      const routeExplanation = await routingApi.explain({
        model: record.requested_model ?? record.model ?? 'unknown-model',
        endpoint: endpointFromPath(record.path),
        source_format: sourceFormatFromPath(record.path),
        tenant_id: record.tenant_id,
        api_key_id: record.api_key_id,
        region: record.client_region ?? null,
        stream: record.stream,
      });
      setExplanation(routeExplanation);
    } catch (actionError) {
      setReplayError(getApiErrorMessage(actionError, t('trafficLab.error.replay')));
    } finally {
      setReplayLoading(false);
    }
  };

  const openSessionDetail = async () => {
    if (!selectedRequestId) {
      setDetailError(t('trafficLab.error.selectSession'));
      setDetailOpen(true);
      return;
    }

    setDetailOpen(true);
    setDetailLoading(true);
    setDetailError(null);
    try {
      const record = await logsApi.getRequest(selectedRequestId);
      if (!record) {
        throw new Error(t('trafficLab.error.requestMissing'));
      }
      setDetailRecord(record);
    } catch (loadError) {
      setDetailError(getApiErrorMessage(loadError, t('trafficLab.error.detail')));
    } finally {
      setDetailLoading(false);
    }
  };

  return (
    <div className="workspace-grid">
      <section className="hero">
        <div>
          <p className="workspace-eyebrow">{t('trafficLab.hero.eyebrow')}</p>
          <h1>{t('trafficLab.hero.title')}</h1>
          <p className="workspace-summary">{t('trafficLab.hero.summary')}</p>
        </div>
        <div className="hero-actions">
          <button className="button button--primary" onClick={() => void handleReplay()}>
            {t('trafficLab.hero.replay')}
          </button>
          <button className="button button--ghost" onClick={() => void openSessionDetail()}>
            {t('trafficLab.hero.inspectSession')}
          </button>
          <button className="button button--ghost" onClick={handleSaveLens}>
            {t('trafficLab.hero.saveLens')}
          </button>
        </div>
      </section>

      {lensStatus ? <div className="status-message status-message--success">{lensStatus}</div> : null}
      {selectedSession ? (
        <div className="status-message status-message--info">
          {t('trafficLab.status.activeSession')} <strong>{selectedSession.request_id}</strong> · {selectedSession.model} · {formatDurationMs(selectedSession.latency_ms)}
        </div>
      ) : null}

      <div className="two-column two-column--70-30">
        <Panel title={t('trafficLab.panel.sessions.title')} subtitle={t('trafficLab.panel.sessions.subtitle')}>
          <div className="inline-actions">
            <input
              name="traffic-session-filter"
              placeholder={t('trafficLab.filter.placeholder')}
              autoComplete="off"
              value={sessionFilter}
              onChange={(event) => updateSearch({ q: event.target.value || null })}
            />
          </div>
          <div className="table-grid table-grid--sessions">
            <div className="table-grid__head">{t('trafficLab.table.session')}</div>
            <div className="table-grid__head">{t('common.model')}</div>
            <div className="table-grid__head">{t('trafficLab.table.decision')}</div>
            <div className="table-grid__head">{t('common.result')}</div>
            <div className="table-grid__head">{t('common.latency')}</div>
            {loading && !data ? <div className="table-grid__cell">{t('trafficLab.loading.sessions')}</div> : null}
            {error && !data ? <div className="table-grid__cell">{error}</div> : null}
            {visibleSessions.flatMap((session) => {
              const selected = session.request_id === selectedRequestId;
              const cellClass = `table-grid__cell ${selected ? 'is-selected' : ''} is-clickable`;
              return [
                <div
                  key={`${session.request_id}-id`}
                  className={`${cellClass} table-grid__cell--strong`}
                  onClick={() => updateSearch({ request: session.request_id })}
                >
                  {session.request_id}
                </div>,
                <div key={`${session.request_id}-model`} className={cellClass} onClick={() => updateSearch({ request: session.request_id })}>
                  {session.model}
                </div>,
                <div key={`${session.request_id}-decision`} className={cellClass} onClick={() => updateSearch({ request: session.request_id })}>
                  {tx(session.decision)}
                </div>,
                <div key={`${session.request_id}-result`} className={cellClass} onClick={() => updateSearch({ request: session.request_id })}>
                  <StatusPill label={tx(session.result)} tone={session.result_tone} />
                </div>,
                <div
                  key={`${session.request_id}-latency`}
                  className={`${cellClass} table-grid__cell--mono`}
                  onClick={() => updateSearch({ request: session.request_id })}
                >
                  {formatDurationMs(session.latency_ms)}
                </div>,
              ];
            })}
          </div>
        </Panel>

        <Panel title={t('trafficLab.panel.windowFacts.title')} subtitle={t('trafficLab.panel.windowFacts.subtitle')}>
          <ul className="fact-list">
            {(data?.compare_facts ?? []).map((fact) => (
              <li key={`${fact.label.key}-${fact.value}`}><span>{tx(fact.label)}</span><strong>{presentFactValue(fact, tx)}</strong></li>
            ))}
            <li><span>{t('trafficLab.fact.liveUpdates')}</span><strong>{live ? t('common.connected') : t('common.paused')}</strong></li>
            <li><span>{t('trafficLab.fact.filter')}</span><strong>{sessionFilter || t('common.none')}</strong></li>
          </ul>
          <div className="sheet-form">
            <label className="sheet-field">
              <span>{t('trafficLab.fact.compareRequest')}</span>
              <select
                value={compareRequestId ?? ''}
                onChange={(event) => updateSearch({ compare: event.target.value || null })}
              >
                <option value="">{t('common.none')}</option>
                {visibleSessions
                  .filter((session) => session.request_id !== selectedRequestId)
                  .map((session) => (
                    <option key={session.request_id} value={session.request_id}>
                      {session.request_id} · {session.model}
                    </option>
                  ))}
              </select>
            </label>
          </div>
          {selectedSession && compareSession ? (
            <div className="detail-grid">
              <div className="detail-grid__row"><span>{t('trafficLab.compare.primary')}</span><strong>{selectedSession.request_id}</strong></div>
              <div className="detail-grid__row"><span>{t('trafficLab.compare.secondary')}</span><strong>{compareSession.request_id}</strong></div>
              <div className="detail-grid__row"><span>{t('trafficLab.compare.latencyDelta')}</span><strong>{formatDurationMs(selectedSession.latency_ms - compareSession.latency_ms)}</strong></div>
              <div className="detail-grid__row"><span>{t('trafficLab.compare.resultDelta')}</span><strong>{tx(selectedSession.result)} vs {tx(compareSession.result)}</strong></div>
            </div>
          ) : null}
        </Panel>
      </div>

      <Panel title={t('trafficLab.panel.trace.title')} subtitle={t('trafficLab.panel.trace.subtitle')}>
        <div className="timeline">
          {(data?.trace ?? []).map((step) => (
            <article key={`${step.label.key}-${step.title.key}`} className="timeline-step">
              <StatusPill label={tx(step.label)} tone={step.tone} />
              <div>
                <strong>{tx(step.title)}</strong>
                <p>{tx(step.detail)}</p>
              </div>
            </article>
          ))}
        </div>
      </Panel>

      <WorkbenchSheet
        open={replayOpen}
        onClose={() => setReplayOpen(false)}
        title={t('trafficLab.replay.title')}
        subtitle={t('trafficLab.replay.subtitle')}
      >
        {replayLoading ? <div className="status-message">{t('trafficLab.loading.replay')}</div> : null}
        {replayError ? <div className="status-message status-message--danger">{replayError}</div> : null}

        {requestRecord ? (
          <section className="sheet-section">
            <h3>{t('trafficLab.replay.selectedRequest')}</h3>
            <div className="detail-grid">
              <div className="detail-grid__row"><span>{t('trafficLab.replay.request')}</span><strong>{requestRecord.request_id}</strong></div>
              <div className="detail-grid__row"><span>{t('common.path')}</span><strong>{requestRecord.path}</strong></div>
              <div className="detail-grid__row"><span>{t('common.model')}</span><strong>{requestRecord.requested_model ?? requestRecord.model ?? t('common.unknown')}</strong></div>
              <div className="detail-grid__row"><span>{t('common.status')}</span><strong>{requestRecord.status}</strong></div>
            </div>
          </section>
        ) : null}

        {explanation ? (
          <>
            <section className="sheet-section">
              <h3>{t('trafficLab.replay.routeExplanation')}</h3>
              <div className="detail-grid">
                <div className="detail-grid__row"><span>{t('common.profile')}</span><strong>{explanation.profile}</strong></div>
                <div className="detail-grid__row"><span>{t('trafficLab.replay.matchedRule')}</span><strong>{explanation.matched_rule ?? t('common.default')}</strong></div>
                <div className="detail-grid__row"><span>{t('trafficLab.replay.winner')}</span><strong>{explanation.selected?.provider ?? t('common.none')}</strong></div>
                <div className="detail-grid__row"><span>{t('trafficLab.replay.credential')}</span><strong>{explanation.selected?.credential_name ?? t('common.none')}</strong></div>
              </div>
            </section>

            <section className="sheet-section">
              <h3>{t('trafficLab.replay.rejections')}</h3>
              {explanation.rejections.length === 0 ? (
                <div className="status-message status-message--success">{t('trafficLab.replay.noRejections')}</div>
              ) : (
                <div className="probe-list">
                  {explanation.rejections.map((rejection) => (
                    <div key={`${rejection.candidate}-${JSON.stringify(rejection.reason)}`} className="probe-check">
                      <span>{rejection.candidate}</span>
                      <strong>{typeof rejection.reason === 'string' ? rejection.reason : t('trafficLab.replay.missingCapability')}</strong>
                    </div>
                  ))}
                </div>
              )}
            </section>
          </>
        ) : null}
      </WorkbenchSheet>

      <WorkbenchSheet
        open={detailOpen}
        onClose={() => setDetailOpen(false)}
        title={t('trafficLab.detail.title')}
        subtitle={t('trafficLab.detail.subtitle')}
      >
        {detailLoading ? <div className="status-message">{t('trafficLab.loading.detail')}</div> : null}
        {detailError ? <div className="status-message status-message--danger">{detailError}</div> : null}

        {detailRecord ? (
          <>
            <section className="sheet-section">
              <h3>{t('trafficLab.detail.requestPosture')}</h3>
              <div className="detail-grid">
                <div className="detail-grid__row"><span>{t('trafficLab.replay.request')}</span><strong>{detailRecord.request_id}</strong></div>
                <div className="detail-grid__row"><span>{t('common.path')}</span><strong>{detailRecord.path}</strong></div>
                <div className="detail-grid__row"><span>{t('common.provider')}</span><strong>{detailRecord.provider ?? t('common.notAvailable')}</strong></div>
                <div className="detail-grid__row"><span>{t('common.tenant')}</span><strong>{detailRecord.tenant_id ?? t('common.global')}</strong></div>
                <div className="detail-grid__row"><span>{t('common.status')}</span><strong>{detailRecord.status}</strong></div>
                <div className="detail-grid__row"><span>{t('common.latency')}</span><strong>{formatDurationMs(detailRecord.latency_ms)}</strong></div>
              </div>
            </section>

            <section className="sheet-section">
              <h3>{t('trafficLab.detail.retryChain')}</h3>
              {detailRecord.attempts && detailRecord.attempts.length > 0 ? (
                <div className="probe-list">
                  {detailRecord.attempts.map((attempt) => (
                    <div key={`${attempt.attempt_index}-${attempt.provider}-${attempt.model}`} className="probe-check">
                      <span>{attempt.provider} / {attempt.model}</span>
                      <strong>{attempt.status ?? t('trafficLab.detail.errorStatus')} · {formatDurationMs(attempt.latency_ms)}</strong>
                    </div>
                  ))}
                </div>
              ) : (
                <div className="status-message">{t('trafficLab.detail.noRetryChain')}</div>
              )}
            </section>

            <section className="sheet-section">
              <h3>{t('trafficLab.detail.payloads')}</h3>
              {!detailRecord.request_body && !detailRecord.upstream_request_body && !detailRecord.response_body && !detailRecord.stream_content_preview ? (
                <div className="status-message">{t('trafficLab.detail.payloadHint')}</div>
              ) : null}
              <PayloadViewer
                title={t('trafficLab.detail.requestBody')}
                payload={detailRecord.request_body}
                emptyMessage={t('trafficLab.detail.noRequestBody')}
                emptyHint={t('trafficLab.detail.payloadEmptyHint')}
              />
              <PayloadViewer
                title={t('trafficLab.detail.upstreamRequest')}
                payload={detailRecord.upstream_request_body}
                emptyMessage={t('trafficLab.detail.noUpstreamBody')}
                emptyHint={t('trafficLab.detail.payloadEmptyHint')}
              />
              <PayloadViewer
                title={t('trafficLab.detail.responseBody')}
                payload={detailRecord.response_body ?? detailRecord.stream_content_preview}
                emptyMessage={t('trafficLab.detail.noResponseBody')}
                emptyHint={t('trafficLab.detail.payloadEmptyHint')}
              />
            </section>
          </>
        ) : null}
      </WorkbenchSheet>
    </div>
  );
}
