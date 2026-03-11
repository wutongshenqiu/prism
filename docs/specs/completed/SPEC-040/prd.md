# SPEC-040: Request Log & Full-Chain Tracing

## Problem

Current request logging has several limitations:

1. **No body capture** -- RequestRecord stores 18+ metadata fields but never captures the client's original request body, the translated upstream body, or the response body. Debugging translation/cloaking issues requires reproducing the request externally.
2. **No per-attempt details** -- `retry_count` is a single counter. When a request retries across multiple providers/credentials, there is no record of which attempts failed, why, or how long each took.
3. **Manual DispatchMeta coupling** -- Dispatch manually builds a `DispatchMeta` struct and injects it into response extensions. The request_logging middleware then destructures it field-by-field into `RequestRecord`. Every new field requires changes in both dispatch and the middleware -- tight coupling that is error-prone.
4. **No OpenTelemetry path** -- Tracing spans exist for log output but carry no structured attributes that could be exported to Jaeger/OTLP collectors. Operators with existing observability stacks cannot correlate gateway requests with their own traces.
5. **No stream content visibility** -- Streaming responses complete asynchronously; the log entry is created before the stream finishes, then patched with usage data. The actual streamed content is never captured, even as a preview.

## Solution

Replace the manual `DispatchMeta` pipeline with a **tracing span-driven** request logging architecture:

- A **`GatewayLogLayer`** (custom `tracing::Layer`) collects structured span attributes on close and assembles `RequestRecord` automatically.
- **Parent-child span hierarchy**: `gateway.request` (entire request lifecycle) → `gateway.attempt` (each upstream attempt).
- **Body capture** at configurable detail levels: metadata-only, standard (truncated bodies), full (complete bodies up to a byte limit).
- **Per-attempt tracking** via `AttemptSummary` records populated from attempt spans.
- **Optional OTel export** via `tracing-opentelemetry` layer, sharing the same span hierarchy.

## Requirements

### R1: Body Capture
- Capture client's original request body (before translation).
- Capture translated upstream request body (after translation + cloaking + payload rules).
- Capture non-streaming response body.
- Capture streaming content preview (first N bytes of accumulated SSE content).
- All body capture respects `max-body-bytes` config limit.

### R2: Per-Attempt Tracking
- Each retry/fallback attempt records: provider, model, credential name, HTTP status, latency, error message, error type.
- `RequestRecord.attempts` contains the ordered list of `AttemptSummary` entries.
- `RequestRecord.total_attempts` reflects the actual count.

### R3: Detail Levels
- **`metadata`** (default) -- Current behavior. No body content captured.
- **`standard`** -- Truncated request/response bodies (first `max-body-bytes` / 4 bytes).
- **`full`** -- Complete bodies up to `max-body-bytes`.

### R4: Span-Driven Collection
- `GatewayLogLayer` listens for `gateway.request` and `gateway.attempt` span closures.
- On `gateway.request` close: assemble `RequestRecord` from span attributes, push to `RequestLogStore`, write to audit backend.
- Eliminates `DispatchMeta` -- dispatch code records data via `Span::current().record()`.

### R5: OTel-Compatible Span Hierarchy
- Span names and attributes follow OpenTelemetry semantic conventions where applicable.
- `gateway.request` carries: `http.method`, `http.route`, `http.status_code`, `rpc.system`, model, request_id.
- `gateway.attempt` carries: provider, model, credential, status, latency, error.
- Optional `tracing-opentelemetry` layer can be added to export spans to OTLP endpoint.

### R6: Configuration
```yaml
audit:
  enabled: true
  dir: ./logs/audit
  retention-days: 30
  detail-level: metadata    # metadata | standard | full
  max-body-bytes: 65536     # max bytes per captured body (default 64KB)
  otel:
    enabled: false
    endpoint: http://localhost:4317
    service-name: prism-gateway
```

## Non-Goals

- Breaking the existing dashboard/WebSocket API -- `RequestRecord` stays backward-compatible (new fields use `skip_serializing_if`).
- Full distributed tracing with trace context propagation to upstream providers (future spec).
- Replacing the in-memory `RequestLogStore` ring buffer with a database.

## Success Criteria

- All existing tests pass.
- `DispatchMeta` removed; dispatch records attributes via span API.
- `request_logging_middleware` simplified to span creation only.
- Body capture works for both streaming and non-streaming at all detail levels.
- Per-attempt details appear in `RequestRecord.attempts`.
- `cargo clippy --workspace --tests -- -D warnings` clean.
- Dashboard and WebSocket continue to work without frontend changes.
