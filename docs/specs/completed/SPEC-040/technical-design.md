# SPEC-040: Technical Design

## Span Hierarchy

```
gateway.request (parent span — created in request_logging middleware)
  ├── gateway.attempt[0] (child span — created per dispatch attempt)
  ├── gateway.attempt[1] (retry)
  └── gateway.attempt[N] (final attempt)
```

- `gateway.request` lives for the entire request lifecycle, including streaming.
- `gateway.attempt` is created and closed within each iteration of the retry loop.

## Core Types

### New / Modified in `crates/core/src/request_record.rs`

Already added in preparation:
- `LogDetailLevel` enum: `Metadata`, `Standard`, `Full`
- `AttemptSummary` struct: `attempt_index`, `provider`, `model`, `credential_name`, `status`, `latency_ms`, `error`, `error_type`
- `RequestRecord` extended with: `request_body`, `upstream_request_body`, `response_body`, `stream_content_preview`, `total_attempts`, `attempts`, `error_type`, `client_region`
- `truncate_body(body, max_bytes)` helper
- `classify_error(ProxyError)` helper

### New: `RequestSpanData` and `AttemptSpanData`

Internal types used by `GatewayLogLayer` to accumulate span attributes:

```rust
// crates/server/src/telemetry/span_data.rs
struct RequestSpanData {
    request_id: String,
    timestamp: DateTime<Utc>,
    method: String,
    path: String,
    stream: bool,
    requested_model: Option<String>,
    request_body: Option<String>,
    upstream_request_body: Option<String>,
    // Final routing result (from last successful attempt)
    provider: Option<String>,
    model: Option<String>,
    credential_name: Option<String>,
    // Response
    status: u16,
    response_body: Option<String>,
    stream_content_preview: Option<String>,
    // Usage & cost
    usage: Option<TokenUsage>,
    cost: Option<f64>,
    // Error
    error: Option<String>,
    error_type: Option<String>,
    // Client context
    api_key_id: Option<String>,
    tenant_id: Option<String>,
    client_ip: Option<String>,
    client_region: Option<String>,
    // Attempts collected from child spans
    attempts: Vec<AttemptSummary>,
}

struct AttemptSpanData {
    attempt_index: u32,
    provider: String,
    model: String,
    credential_name: Option<String>,
    status: Option<u16>,
    start: Instant,
    error: Option<String>,
    error_type: Option<String>,
}
```

## GatewayLogLayer

New module: `crates/server/src/telemetry/gateway_log_layer.rs`

A custom `tracing_subscriber::Layer` implementation:

```rust
pub struct GatewayLogLayer {
    request_logs: Arc<RequestLogStore>,
    audit: Arc<dyn AuditBackend>,
    detail_level: LogDetailLevel,
    max_body_bytes: usize,
}
```

### Layer Behavior

1. **`on_new_span`** -- For `gateway.request` spans: initialize `RequestSpanData` in span extensions. For `gateway.attempt` spans: initialize `AttemptSpanData`.
2. **`on_record`** -- Update span data fields when `Span::record()` is called.
3. **`on_close`** -- For `gateway.attempt`: finalize `AttemptSpanData`, push into parent's `attempts` vec. For `gateway.request`: convert `RequestSpanData` → `RequestRecord`, push to `RequestLogStore`, write to audit backend.

### Span Attribute Recording

Dispatch code records attributes via the tracing API instead of building `DispatchMeta`:

```rust
// In dispatch — after credential selection
Span::current().record("provider", &provider_name);
Span::current().record("model", &model_name);
Span::current().record("credential_name", &cred.name);

// In dispatch — after response
Span::current().record("status", status.as_u16());
```

## Config Changes

Extend `AuditConfig` in `crates/core/src/audit.rs`:

```rust
pub struct AuditConfig {
    pub enabled: bool,
    pub dir: String,
    pub retention_days: u32,
    pub detail_level: LogDetailLevel,      // NEW (default: Metadata)
    pub max_body_bytes: usize,             // NEW (default: 65536)
    pub otel: Option<OtelConfig>,          // NEW (default: None)
}

pub struct OtelConfig {
    pub enabled: bool,
    pub endpoint: String,
    pub service_name: String,
}
```

## Files Changed

| File | Change |
|------|--------|
| `crates/core/src/request_record.rs` | Already has `LogDetailLevel`, `AttemptSummary`, extended `RequestRecord`, `truncate_body`, `classify_error` |
| `crates/core/src/audit.rs` | Add `detail_level`, `max_body_bytes`, `otel` to `AuditConfig` |
| `crates/server/src/telemetry/mod.rs` | **NEW** -- module root, re-exports |
| `crates/server/src/telemetry/span_data.rs` | **NEW** -- `RequestSpanData`, `AttemptSpanData` |
| `crates/server/src/telemetry/gateway_log_layer.rs` | **NEW** -- `GatewayLogLayer` impl |
| `crates/server/src/middleware/request_logging.rs` | Simplify: create `gateway.request` span, attach to request; remove `DispatchMeta` extraction and `RequestRecord` assembly |
| `crates/server/src/dispatch.rs` | Remove `DispatchMeta` struct; record attributes via `Span::current().record()` |
| `crates/server/src/dispatch/helpers.rs` | Record body capture and usage via span attributes |
| `crates/server/src/dispatch/streaming.rs` | Pass request span into `with_usage_capture`; accumulate stream content preview; close span on `Drop` |
| `crates/server/src/dispatch/retry.rs` | Create `gateway.attempt` child span per attempt |
| `crates/server/src/lib.rs` | Register `GatewayLogLayer` in tracing subscriber stack |
| `src/app.rs` | Pass `AuditConfig` to telemetry layer setup |

## Streaming Integration

Streaming responses require special handling because the response body is sent incrementally:

1. `gateway.request` span is created in middleware and entered.
2. Dispatch returns an SSE stream response; the span is **not** closed yet.
3. The span guard is moved into the stream's `UsageCaptureBody` wrapper (existing pattern in `streaming.rs`).
4. As SSE chunks arrive, stream content preview is accumulated (up to `max_body_bytes`).
5. On stream completion (or drop), the wrapper:
   - Records final usage, cost, and stream preview into the span.
   - Drops the span guard, triggering `GatewayLogLayer::on_close`.

## Migration Plan

### Phase 1: Config & Types
- Add `detail_level`, `max_body_bytes`, `otel` to `AuditConfig`
- Types already in place (`LogDetailLevel`, `AttemptSummary`, extended `RequestRecord`)

### Phase 2: GatewayLogLayer
- Implement `telemetry/` module with `GatewayLogLayer`
- Register in tracing subscriber stack
- Unit tests for span data collection

### Phase 3: Dispatch Refactor
- Replace `DispatchMeta` with span recording in dispatch, helpers, streaming, retry
- Create `gateway.attempt` spans in retry loop
- Capture bodies based on detail level

### Phase 4: Middleware Simplification
- Simplify `request_logging_middleware` to only create the `gateway.request` span
- Remove `DispatchMeta` struct entirely
- Integration tests

## Backward Compatibility

- `RequestRecord` new fields all have `#[serde(default, skip_serializing_if)]` -- existing JSON consumers see no change when fields are empty.
- Dashboard WebSocket pushes `RequestRecord` -- new fields appear only when `detail_level > Metadata`.
- `AuditConfig` new fields have defaults -- existing `config.yaml` files work without changes.
- `retry_count` field kept as deprecated alias for `total_attempts` during transition (removed in future).
