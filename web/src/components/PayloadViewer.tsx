import { useMemo, useState } from 'react';
import { useI18n } from '../i18n';
import { inspectPayload } from '../lib/payloadInspector';

interface PayloadViewerProps {
  title: string;
  payload: unknown;
  emptyMessage: string;
  emptyHint?: string | null;
}

function scalarTone(value: unknown) {
  if (typeof value === 'number') return 'number';
  if (typeof value === 'boolean') return 'boolean';
  if (value === null) return 'null';
  return 'string';
}

function renderScalar(value: unknown) {
  if (typeof value === 'string') {
    return JSON.stringify(value);
  }
  if (value === null) {
    return 'null';
  }
  return String(value);
}

function StructuredValue({ value }: { value: unknown }) {
  if (Array.isArray(value)) {
    if (value.length === 0) {
      return <span className="json-tree__scalar json-tree__scalar--null">[]</span>;
    }

    return (
      <div className="json-tree">
        {value.map((item, index) => (
          <div key={`array-${index}`} className="json-tree__row">
            <span className="json-tree__key">[{index}]</span>
            <div className="json-tree__branch">
              <StructuredValue value={item} />
            </div>
          </div>
        ))}
      </div>
    );
  }

  if (value && typeof value === 'object') {
    const entries = Object.entries(value);
    if (entries.length === 0) {
      return <span className="json-tree__scalar json-tree__scalar--null">{'{}'}</span>;
    }

    return (
      <div className="json-tree">
        {entries.map(([key, entryValue]) => (
          <div key={key} className="json-tree__row">
            <span className="json-tree__key">{key}</span>
            <div className="json-tree__branch">
              <StructuredValue value={entryValue} />
            </div>
          </div>
        ))}
      </div>
    );
  }

  return (
    <span className={`json-tree__scalar json-tree__scalar--${scalarTone(value)}`}>
      {renderScalar(value)}
    </span>
  );
}

function PayloadViewerPanel({
  title,
  emptyMessage,
  emptyHint,
  inspected,
}: Omit<PayloadViewerProps, 'payload'> & { inspected: ReturnType<typeof inspectPayload> }) {
  const { t } = useI18n();
  const [mode, setMode] = useState<'visual' | 'raw'>('visual');

  return (
    <div className="code-block code-block--payload">
      <div className="code-block__header">
        <strong>{title}</strong>
        {inspected ? (
          <div className="segmented-control" role="tablist" aria-label={title}>
            <button
              type="button"
              className={`segmented-control__item ${mode === 'visual' ? 'is-active' : ''}`}
              onClick={() => setMode('visual')}
            >
              {t('common.visual')}
            </button>
            <button
              type="button"
              className={`segmented-control__item ${mode === 'raw' ? 'is-active' : ''}`}
              onClick={() => setMode('raw')}
            >
              {t('common.rawView')}
            </button>
          </div>
        ) : null}
      </div>

      {!inspected ? (
        <div className="payload-empty">
          <div className="status-message">{emptyMessage}</div>
          {emptyHint ? <p className="payload-empty__hint">{emptyHint}</p> : null}
        </div>
      ) : null}

      {inspected && mode === 'visual' && inspected.kind === 'json' ? (
        <StructuredValue value={inspected.value} />
      ) : null}

      {inspected && mode === 'visual' && inspected.kind === 'events' ? (
        <div className="event-stream">
          {inspected.events.length === 0 ? (
            <div className="status-message">{t('trafficLab.detail.noRenderableEvents')}</div>
          ) : (
            inspected.events.map((event) => (
              <article key={`${event.index}-${event.event ?? 'message'}`} className="event-stream__item">
                <div className="event-stream__meta">
                  <strong>{t('common.event')} #{event.index}</strong>
                  <span>{event.event ?? t('common.default')}</span>
                </div>
                <StructuredValue value={event.data} />
              </article>
            ))
          )}
          {inspected.removedDone ? (
            <p className="payload-empty__hint">{t('trafficLab.detail.doneMarkerHidden')}</p>
          ) : null}
        </div>
      ) : null}

      {inspected && mode === 'visual' && inspected.kind === 'text' ? (
        <pre>{inspected.value}</pre>
      ) : null}

      {inspected && mode === 'raw' ? <pre>{inspected.raw}</pre> : null}
    </div>
  );
}

export function PayloadViewer({
  title,
  payload,
  emptyMessage,
  emptyHint,
}: PayloadViewerProps) {
  const inspected = useMemo(() => inspectPayload(payload), [payload]);
  const resetKey = inspected
    ? `${inspected.kind}:${inspected.raw.length}:${inspected.raw.slice(0, 96)}`
    : `empty:${typeof payload}:${payload === null ? 'null' : 'value'}`;

  return (
    <PayloadViewerPanel
      key={resetKey}
      title={title}
      emptyMessage={emptyMessage}
      emptyHint={emptyHint}
      inspected={inspected}
    />
  );
}
