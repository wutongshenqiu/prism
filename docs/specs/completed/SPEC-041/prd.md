# SPEC-041: LogStore Abstraction & Dashboard UI Overhaul

## Problem

The current request logging and dashboard have several architectural and UX limitations:

1. **Write/query path split** -- `GatewayLogLayer` writes to both `RequestLogStore` (in-memory ring buffer) and `AuditBackend` (file/noop). Dashboard queries only hit the in-memory store. Even if a new audit backend (SLS, ClickHouse, etc.) is implemented, the dashboard cannot query it.
2. **No unified LogStore trait** -- `RequestLogStore` is a concrete struct, not a trait. Swapping to an external log backend requires rewriting all consumers (dashboard handlers, WebSocket, GatewayLogLayer).
3. **Default detail level is Metadata** -- Request/response bodies are not captured by default, making the dashboard log detail view empty and confusing.
4. **Weak query capabilities** -- Only 5 filter dimensions (provider, model, status, date_from, date_to). Missing: request_id lookup, tenant_id, api_key_id, error_type, latency range, keyword search, stream filter.
5. **No single-record API** -- Cannot fetch a specific request by ID. No shareable URL for a specific log entry.
6. **Dashboard UI limitations:**
   - Request log detail shown via inline row expansion -- poor readability, cannot compare entries.
   - JSON bodies displayed as raw `<pre>` text -- no syntax highlighting, folding, or copy.
   - Overview and Metrics pages overlap significantly -- redundant charts.
   - No time range selector -- metrics are either real-time or all-time with no windowing.
   - No Live/Pause toggle -- real-time log push causes list jumping during analysis.
   - Filter inputs are free-text instead of dropdowns -- user must remember exact provider/model names.
   - No filter options API -- frontend cannot populate dropdown choices.
   - Stats computed partially on frontend (metricsStore) instead of backend.
   - Missing Top-N rankings (models, errors), cost metrics, percentile latencies.

## Solution

### Backend: LogStore Trait Abstraction

Replace `RequestLogStore` + `AuditBackend` with a unified `LogStore` trait:

```rust
#[async_trait]
pub trait LogStore: Send + Sync {
    async fn push(&self, entry: RequestRecord);
    async fn get(&self, request_id: &str) -> Option<RequestRecord>;
    async fn query(&self, q: &LogQuery) -> LogPage;
    async fn stats(&self, q: &StatsQuery) -> LogStats;
    async fn filter_options(&self) -> FilterOptions;
    fn subscribe(&self) -> broadcast::Receiver<RequestRecord>;
}
```

Two implementations:
- **`InMemoryLogStore`** (default): ring buffer + optional file audit. Drop-in replacement for current behavior with extended query support.
- **External backends** (SLS, etc.): implement the same trait, dashboard queries go through the external service. Broadcast channel still provides real-time WebSocket fanout.

### Frontend: Dashboard UI Overhaul

- **Merge Overview + Metrics** into a single Dashboard page with a global time range picker.
- **Request Logs** page rewritten with:
  - Drawer-based detail panel (replaces inline expand).
  - Rich filter bar with dropdown selectors populated from backend.
  - Live/Pause toggle for real-time updates.
  - URL-addressable log detail (`/request-logs?id=xxx`).
  - Collapsible JSON viewer with syntax highlighting and copy.
- **New backend-computed stats**: Top models, top errors, percentile latencies, cost totals, time series bucketing.

## Requirements

### R1: LogStore Trait

- Define `LogStore` async trait in `crates/core/src/request_log.rs`.
- All dashboard handlers, WebSocket handler, and GatewayLogLayer use `Arc<dyn LogStore>`.
- Remove `AuditBackend` trait -- file audit is an internal detail of `InMemoryLogStore`.
- `AppState` holds `log_store: Arc<dyn LogStore>` instead of separate `request_logs` + `audit`.

### R2: Extended LogQuery

Expand `LogQuery` with:
- `request_id: Option<String>` -- exact match
- `tenant_id: Option<String>`
- `api_key_id: Option<String>`
- `error_type: Option<String>`
- `stream: Option<bool>`
- `latency_min: Option<u64>`, `latency_max: Option<u64>`
- `keyword: Option<String>` -- substring search across body/error fields
- `sort_by: Option<SortField>`, `sort_order: Option<SortOrder>`

### R3: Rich Stats

`LogStats` returned by `stats()` includes:
- Percentile latencies (p50, p95, p99).
- Total cost and total tokens.
- Time series buckets (for chart rendering).
- Top models by request count.
- Top error types by count.
- Provider distribution.
- Status code distribution (2xx/4xx/5xx counts).

### R4: FilterOptions API

- `GET /api/dashboard/logs/filters` returns distinct providers, models, error types, tenant IDs currently in the store.
- Frontend uses these to populate dropdown selectors.

### R5: Single Record API

- `GET /api/dashboard/logs/:id` returns a single `RequestRecord` by request_id.
- Returns 404 if not found.

### R6: InMemoryLogStore

- Ring buffer with configurable capacity (default 10,000).
- Optional file audit writer (JSONL with daily rotation + retention cleanup).
- Broadcast channel for WebSocket fanout.
- Supports all extended query/stats/filter_options methods.

### R7: Configuration

```yaml
log-store:
  backend: memory           # "memory" (default)
  capacity: 10000
  detail-level: full        # metadata | standard | full (default: full)
  max-body-bytes: 32768
  file-audit:
    enabled: false
    dir: ./logs/audit
    retention-days: 30
```

Old `audit:` config key continues to work as an alias (mapped internally).

### R8: Dashboard Page Merge

- Remove `/metrics` route and `Metrics.tsx`.
- Remove `/` Overview page. New `/` route renders `Dashboard.tsx`.
- Dashboard page contains:
  - Time range picker (5m, 15m, 1h, 6h, 24h).
  - 6 metric cards (Requests, Errors, Tokens, Cost, Active Providers, Avg Latency) with period-over-period comparison.
  - Request trend line chart.
  - Provider distribution pie chart.
  - Latency distribution bar chart.
  - Top Models list.
  - Recent Errors list.
- All stats fetched from backend `GET /api/dashboard/logs/stats?from=&to=`.

### R9: Request Logs UI Rewrite

- **Drawer panel**: clicking a row slides in a right-side drawer (~50% width) showing full detail.
- **Drawer content sections**: Overview metadata, Token Usage, Retry Attempts timeline, Request Body (JSON viewer), Upstream Body, Response Body / Stream Preview, Error detail.
- **JSON viewer component**: collapsible tree with syntax coloring and copy-to-clipboard.
- **Filter bar**: dropdown selectors for Provider, Model, Status, Error Type, Tenant, Stream. Datetime range picker with second precision. Latency min/max inputs. Keyword search input.
- **Live/Pause toggle**: Live mode adds incoming WebSocket logs to list. Pause mode freezes the list.
- **URL state**: selecting a log updates URL to `/request-logs?id=xxx`. Opening that URL auto-fetches and opens the drawer.

### R10: Navigation Update

Sidebar nav items: Dashboard, Request Logs, Providers, Auth Keys, Routing, System, App Logs (7 items, down from 8).

## Non-Goals

- SLS/ClickHouse/external backend implementation -- trait is designed for it but implementation is a separate spec.
- Dark mode.
- Log export to CSV/JSON (future enhancement).
- Full-text search indexing -- keyword search is substring match on in-memory data.

## Success Criteria

- All existing Rust tests pass (`cargo test --workspace`).
- All existing frontend tests pass (`npm test`).
- `cargo clippy --workspace --tests -- -D warnings` clean.
- `npx tsc --noEmit` clean.
- Dashboard renders merged page with time range picker and all charts/rankings.
- Request Logs page uses drawer for detail, with dropdown filters and Live/Pause.
- `GET /api/dashboard/logs/:id` returns correct record.
- `GET /api/dashboard/logs/filters` returns filter options.
- `GET /api/dashboard/logs/stats` returns extended stats with time range support.
- Default `detail-level` is `full` -- request/response bodies are captured.
- WebSocket real-time push continues to work.
