export interface PayloadEvent {
  index: number;
  event: string | null;
  data: unknown;
  raw: string;
}

export type InspectedPayload =
  | { kind: 'json'; raw: string; value: unknown }
  | { kind: 'events'; raw: string; events: PayloadEvent[]; removedDone: boolean }
  | { kind: 'text'; raw: string; value: string };

function tryParseJson(value: string): { ok: boolean; value: unknown } {
  try {
    return { ok: true, value: JSON.parse(value) as unknown };
  } catch {
    return { ok: false, value: null };
  }
}

function inspectSsePayload(raw: string): InspectedPayload | null {
  if (!/(^|\n)(event|data):/m.test(raw)) {
    return null;
  }

  const events: PayloadEvent[] = [];
  let currentEvent: string | null = null;
  let dataLines: string[] = [];
  let removedDone = false;

  const flush = () => {
    if (dataLines.length === 0) {
      currentEvent = null;
      return;
    }

    const rawData = dataLines.join('\n').trim();
    if (!rawData) {
      currentEvent = null;
      dataLines = [];
      return;
    }

    if (rawData === '[DONE]') {
      removedDone = true;
      currentEvent = null;
      dataLines = [];
      return;
    }

    const parsed = tryParseJson(rawData);

    events.push({
      index: events.length + 1,
      event: currentEvent,
      data: parsed.ok ? parsed.value : rawData,
      raw: rawData,
    });

    currentEvent = null;
    dataLines = [];
  };

  for (const line of raw.split(/\r?\n/)) {
    if (line.trim() === '') {
      flush();
      continue;
    }

    if (line.startsWith(':')) {
      continue;
    }

    if (line.startsWith('event:')) {
      currentEvent = line.slice('event:'.length).trim() || null;
      continue;
    }

    if (line.startsWith('data:')) {
      dataLines.push(line.slice('data:'.length).trimStart());
      continue;
    }
  }

  flush();

  if (events.length === 0 && !removedDone) {
    return null;
  }

  return {
    kind: 'events',
    raw,
    events,
    removedDone,
  };
}

export function inspectPayload(payload: unknown): InspectedPayload | null {
  if (payload == null) {
    return null;
  }

  if (typeof payload === 'string') {
    const trimmed = payload.trim();
    if (!trimmed) {
      return null;
    }

    const parsedJson = tryParseJson(trimmed);
    if (parsedJson.ok) {
      return {
        kind: 'json',
        raw: JSON.stringify(parsedJson.value, null, 2),
        value: parsedJson.value,
      };
    }

    const ssePayload = inspectSsePayload(payload);
    if (ssePayload) {
      return ssePayload;
    }

    return {
      kind: 'text',
      raw: payload,
      value: payload,
    };
  }

  return {
    kind: 'json',
    raw: JSON.stringify(payload, null, 2),
    value: payload,
  };
}
