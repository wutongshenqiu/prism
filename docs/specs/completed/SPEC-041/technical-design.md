# SPEC-041: Technical Design

## Architecture Overview

```
                         ┌─────────────────────┐
                         │   GatewayLogLayer    │
                         │  (tracing::Layer)    │
                         └─────────┬───────────┘
                                   │ on_close: log_store.push(record)
                                   ▼
                        ┌─────────────────────┐
                        │  Arc<dyn LogStore>   │◄────── AppState.log_store
                        └─────────┬───────────┘
                                  │
              ┌───────────────────┼───────────────────┐
              ▼                   ▼                   ▼
   ┌──────────────────┐ ┌─────────────────┐ ┌──────────────────┐
   │ InMemoryLogStore │ │  (future) SLS   │ │ (future) SQLite  │
   │                  │ │   LogStore      │ │   LogStore       │
   │ - VecDeque ring  │ │                 │ │                  │
   │ - broadcast tx   │ │ - PutLogs       │ │ - INSERT         │
   │ - FileAuditWriter│ │ - GetLogs       │ │ - SELECT         │
   └──────────────────┘ └─────────────────┘ └──────────────────┘
         │   │
         │   └──── query()/get()/stats()/filter_options()
         │              ▲
         │              │
         │   ┌──────────┴──────────┐
         │   │ Dashboard Handlers  │
         │   │ GET /logs           │
         │   │ GET /logs/:id       │
         │   │ GET /logs/stats     │
         │   │ GET /logs/filters   │
         │   └─────────────────────┘
         │
         └──── subscribe()
                    ▲
                    │
              ┌─────┴──────┐
              │  WebSocket  │
              │  Handler    │
              └────────────┘
```

## 1. Backend Changes

### 1.1 LogStore Trait (`crates/core/src/request_log.rs`)

Complete rewrite. Delete current `RequestLogStore` struct.

```rust
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use crate::request_record::RequestRecord;

// ── Query types ──

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SortField {
    Timestamp,
    Latency,
    Cost,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SortOrder {
    Asc,
    Desc,
}

#[derive(Debug, Default, Deserialize)]
pub struct LogQuery {
    pub page: Option<usize>,
    pub page_size: Option<usize>,
    pub request_id: Option<String>,
    pub tenant_id: Option<String>,
    pub api_key_id: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub status: Option<String>,        // "2xx"/"4xx"/"5xx" or specific code
    pub error_type: Option<String>,
    pub stream: Option<bool>,
    pub from: Option<i64>,             // timestamp ms
    pub to: Option<i64>,
    pub latency_min: Option<u64>,
    pub latency_max: Option<u64>,
    pub keyword: Option<String>,
    pub sort_by: Option<SortField>,
    pub sort_order: Option<SortOrder>,
}

#[derive(Debug, Serialize)]
pub struct LogPage {
    pub data: Vec<RequestRecord>,
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
    pub total_pages: usize,
}

#[derive(Debug, Default, Deserialize)]
pub struct StatsQuery {
    pub from: Option<i64>,
    pub to: Option<i64>,
    pub provider: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LogStats {
    pub total_entries: usize,
    pub error_count: usize,
    pub avg_latency_ms: u64,
    pub p50_latency_ms: u64,
    pub p95_latency_ms: u64,
    pub p99_latency_ms: u64,
    pub total_cost: f64,
    pub total_tokens: u64,
    pub time_series: Vec<TimeSeriesBucket>,
    pub top_models: Vec<ModelStats>,
    pub top_errors: Vec<ErrorStats>,
    pub provider_distribution: Vec<ProviderDistribution>,
    pub status_distribution: StatusDistribution,
}

#[derive(Debug, Serialize)]
pub struct TimeSeriesBucket {
    pub timestamp: String,      // ISO 8601 bucket start
    pub requests: u64,
    pub errors: u64,
    pub avg_latency_ms: u64,
    pub tokens: u64,
    pub cost: f64,
}

#[derive(Debug, Serialize)]
pub struct ModelStats {
    pub model: String,
    pub requests: u64,
    pub avg_latency_ms: u64,
    pub total_tokens: u64,
    pub total_cost: f64,
}

#[derive(Debug, Serialize)]
pub struct ErrorStats {
    pub error_type: String,
    pub count: u64,
    pub last_seen: String,
}

#[derive(Debug, Serialize)]
pub struct ProviderDistribution {
    pub provider: String,
    pub requests: u64,
    pub percentage: f64,
}

#[derive(Debug, Serialize)]
pub struct StatusDistribution {
    pub success: u64,   // 2xx
    pub client_error: u64, // 4xx
    pub server_error: u64, // 5xx
}

#[derive(Debug, Serialize)]
pub struct FilterOptions {
    pub providers: Vec<String>,
    pub models: Vec<String>,
    pub error_types: Vec<String>,
    pub tenant_ids: Vec<String>,
}

// ── Trait ──

#[async_trait]
pub trait LogStore: Send + Sync {
    async fn push(&self, entry: RequestRecord);
    async fn get(&self, request_id: &str) -> Option<RequestRecord>;
    async fn query(&self, q: &LogQuery) -> LogPage;
    async fn stats(&self, q: &StatsQuery) -> LogStats;
    async fn filter_options(&self) -> FilterOptions;
    fn subscribe(&self) -> broadcast::Receiver<RequestRecord>;

    /// Update usage/cost for a streaming request after completion.
    async fn update_usage(
        &self,
        request_id: &str,
        usage: crate::request_record::TokenUsage,
        cost: Option<f64>,
    );
}
```

### 1.2 InMemoryLogStore (`crates/core/src/memory_log_store.rs`)

New file. Implements `LogStore` for the in-memory ring buffer.

Key internals:
- `entries: RwLock<VecDeque<RequestRecord>>` -- ring buffer with configurable capacity.
- `tx: broadcast::Sender<RequestRecord>` -- fanout for WebSocket.
- `file_writer: Option<Mutex<FileAuditWriter>>` -- optional JSONL file sink.

Query implementation:
- `query()`: iterate `entries` in reverse (newest first), apply all filters, paginate.
- `get()`: `entries.iter().rfind(|e| e.request_id == id)`.
- `stats()`: single pass over filtered entries, compute percentiles via sorted latency vec.
- `filter_options()`: collect distinct values via `HashSet`.

`keyword` filter: substring match on `request_body`, `response_body`, `stream_content_preview`, `error`, `upstream_request_body`.

Time series bucketing in `stats()`:
- Determine bucket interval from query time range (< 15m → 10s, < 1h → 1m, < 6h → 5m, < 24h → 15m, else → 1h).
- Group filtered entries into buckets, compute per-bucket aggregates.

### 1.3 FileAuditWriter (`crates/core/src/file_audit.rs`)

Extract from old `audit.rs`. Simple struct, NOT a trait:

```rust
pub struct FileAuditWriter {
    dir: String,
    retention_days: u32,
    writer: Mutex<Option<BufWriter<File>>>,
    current_date: Mutex<NaiveDate>,
}

impl FileAuditWriter {
    pub fn new(config: &FileAuditConfig) -> io::Result<Self> { ... }
    pub async fn write(&self, entry: &RequestRecord) { ... }
    pub fn spawn_cleanup_task(dir: String, retention_days: u32) { ... }
}
```

### 1.4 Config Changes (`crates/core/src/config.rs`)

```rust
// Replace AuditConfig with:
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct LogStoreConfig {
    pub backend: LogStoreBackend,       // "memory" (default)
    pub capacity: usize,                // ring buffer size, default 10000
    pub detail_level: LogDetailLevel,   // default Full
    pub max_body_bytes: usize,          // default 32768
    pub file_audit: FileAuditConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LogStoreBackend {
    Memory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct FileAuditConfig {
    pub enabled: bool,
    pub dir: String,
    pub retention_days: u32,
}
```

In `Config` struct: replace `pub audit: AuditConfig` with `pub log_store: LogStoreConfig`.

Support old `audit:` key via `#[serde(alias = "audit")]` or a custom deserializer.

### 1.5 AppState (`crates/server/src/lib.rs`)

```rust
pub struct AppState {
    pub config: Arc<ArcSwap<Config>>,
    pub router: Arc<CredentialRouter>,
    pub executors: Arc<ExecutorRegistry>,
    pub translators: Arc<TranslatorRegistry>,
    pub metrics: Arc<Metrics>,
    pub log_store: Arc<dyn LogStore>,     // ← replaces request_logs + audit
    pub config_path: Arc<Mutex<String>>,
    pub rate_limiter: Arc<CompositeRateLimiter>,
    pub cost_calculator: Arc<CostCalculator>,
    pub response_cache: Option<Arc<dyn ResponseCacheBackend>>,
    pub start_time: Instant,
}
```

### 1.6 GatewayLogLayer (`crates/server/src/telemetry/gateway_log_layer.rs`)

```rust
pub struct GatewayLogLayer {
    log_store: Arc<dyn LogStore>,    // ← replaces request_logs + audit
}

// on_close:
if name == REQUEST_SPAN_NAME {
    let data = span.extensions_mut().remove::<RequestSpanData>();
    if let Some(data) = data {
        let record = data.into_request_record();
        let store = self.log_store.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                store.push(record).await;
            });
        }
    }
}
```

### 1.7 Dashboard Handlers (`crates/server/src/handler/dashboard/logs.rs`)

```rust
pub async fn query_logs(
    State(state): State<AppState>,
    Query(query): Query<LogQuery>,
) -> impl IntoResponse {
    let page = state.log_store.query(&query).await;
    (StatusCode::OK, Json(page))
}

pub async fn get_log(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.log_store.get(&id).await {
        Some(record) => Json(record).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

pub async fn log_stats(
    State(state): State<AppState>,
    Query(query): Query<StatsQuery>,
) -> impl IntoResponse {
    let stats = state.log_store.stats(&query).await;
    (StatusCode::OK, Json(stats))
}

pub async fn filter_options(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let options = state.log_store.filter_options().await;
    (StatusCode::OK, Json(options))
}
```

Router addition in `lib.rs`:
```rust
.route("/logs/{id}", get(logs::get_log))
.route("/logs/filters", get(logs::filter_options))
```

Note: `/logs/filters` must be registered BEFORE `/logs/{id}` to avoid `filters` being captured as an `{id}` parameter.

### 1.8 WebSocket Handler

```rust
// Change:
let mut log_rx = state.request_logs.subscribe();
// To:
let mut log_rx = state.log_store.subscribe();
```

### 1.9 Dispatch (`crates/server/src/dispatch.rs`)

Replace all `config.audit.*` references with `config.log_store.*`:
- `config.audit.detail_level` → `config.log_store.detail_level`
- `config.audit.max_body_bytes` → `config.log_store.max_body_bytes`

### 1.10 main.rs / app.rs

```rust
// Build log store based on config
let log_store: Arc<dyn LogStore> = {
    let ls_config = &config.log_store;
    let file_writer = if ls_config.file_audit.enabled {
        Some(FileAuditWriter::new(&ls_config.file_audit)?)
    } else {
        None
    };
    Arc::new(InMemoryLogStore::new(ls_config.capacity, file_writer))
};

// GatewayLogLayer takes only log_store
let log_layer = GatewayLogLayer::new(log_store.clone());

// AppState uses log_store
let state = AppState { log_store, ... };
```

### 1.11 Delete `crates/core/src/audit.rs`

Remove `AuditBackend` trait, `FileAuditBackend`, `NoopAuditBackend`, `AuditConfig`. All replaced by `LogStore` + `FileAuditWriter` + `LogStoreConfig`.

## 2. Frontend Changes

### 2.1 Types (`web/src/types/index.ts`)

Add/modify:

```typescript
// Extended filter
interface RequestLogFilter {
  request_id?: string;
  provider?: string;
  model?: string;
  status?: string;
  error_type?: string;
  stream?: boolean;
  tenant_id?: string;
  api_key_id?: string;
  from?: string;       // ISO datetime or timestamp ms
  to?: string;
  latency_min?: number;
  latency_max?: number;
  keyword?: string;
  sort_by?: 'timestamp' | 'latency' | 'cost';
  sort_order?: 'asc' | 'desc';
}

// Stats (from backend)
interface LogStats {
  total_entries: number;
  error_count: number;
  avg_latency_ms: number;
  p50_latency_ms: number;
  p95_latency_ms: number;
  p99_latency_ms: number;
  total_cost: number;
  total_tokens: number;
  time_series: TimeSeriesBucket[];
  top_models: ModelStats[];
  top_errors: ErrorStats[];
  provider_distribution: ProviderDistribution[];
  status_distribution: StatusDistribution;
}

interface TimeSeriesBucket { ... }
interface ModelStats { ... }
interface ErrorStats { ... }
interface StatusDistribution { success: number; client_error: number; server_error: number; }

interface FilterOptions {
  providers: string[];
  models: string[];
  error_types: string[];
  tenant_ids: string[];
}

type TimeRange = '5m' | '15m' | '1h' | '6h' | '24h';
```

### 2.2 API Service (`web/src/services/api.ts`)

```typescript
export const logsApi = {
  list: (page, pageSize, filters) => { /* extended params */ },
  getById: (id: string) => api.get(`/logs/${id}`),
  stats: (query?: { from?: number; to?: number; provider?: string; model?: string }) =>
    api.get('/logs/stats', { params: query }),
  filters: () => api.get('/logs/filters'),
};
```

### 2.3 Stores

**`metricsStore.ts`** -- Simplified:
- Remove `timeSeries`, `providerDistribution`, `latencyBuckets`, `addTimeSeriesPoint`, `setProviderDistribution`, `setLatencyBuckets`.
- Add `stats: LogStats | null`, `timeRange: TimeRange`, `fetchStats(timeRange)`.
- `setSnapshot()` still receives WebSocket metrics.

**`logsStore.ts`** -- Extended:
- Add `filterOptions: FilterOptions`, `fetchFilterOptions()`.
- Add `selectedLogId`, `selectedLog`, `isDrawerOpen`, `isLoadingDetail`.
- Add `isLive: boolean`, `toggleLive()`.
- Add `openDrawer(id)`, `closeDrawer()`, `fetchLogDetail(id)`.
- `addLog()` only updates state when `isLive === true`.

### 2.4 New Components

**`LogDrawer.tsx`**: Fixed-position right panel, ~50% width. Sections: Overview grid, Token Usage, Retry Attempts, Request Body (JsonViewer), Upstream Body, Response/Stream body, Error detail. Close button + ESC key handler.

**`JsonViewer.tsx`**: Recursive tree renderer. Collapsible objects/arrays. Syntax colors via CSS classes. Copy-to-clipboard button per section.

**`TimeRangePicker.tsx`**: Button group: 5m | 15m | 1h | 6h | 24h. Active state styling. Calls parent onChange with new range.

**`FilterSelect.tsx`**: Wrapper around `<select>` with options from FilterOptions. Supports optional "All" default.

**`TopList.tsx`**: Ordered list with rank number, label, and value. Used for top models and recent errors.

### 2.5 Pages

**`Dashboard.tsx`** (new, replaces Overview + Metrics):
- Time range picker in header.
- 6 metric cards using `stats` data.
- Request trend LineChart from `stats.time_series`.
- 2-column grid: Provider PieChart + Top Models.
- 2-column grid: Latency BarChart + Recent Errors.

**`RequestLogs.tsx`** (rewrite):
- Filter bar with FilterSelect dropdowns + datetime inputs + latency range + keyword search.
- Table with click-to-open-drawer.
- LogDrawer component.
- Live/Pause toggle button.
- URL search param sync (`?id=xxx` opens drawer on mount).

### 2.6 Layout + Routing

**`Layout.tsx`**: Update navItems to 7 items (remove Metrics).

**`App.tsx`**: Remove `/metrics` route. Index route renders `Dashboard`. Remove `Overview.tsx` import.

### 2.7 Styles (`App.css`)

Add sections:
- `.drawer-overlay`, `.drawer`, `.drawer--open`, `.drawer-header`, `.drawer-body`, `.drawer-section`.
- `.json-viewer`, `.json-key`, `.json-string`, `.json-number`, `.json-boolean`, `.json-null`, `.json-toggle`.
- `.time-range-picker`, `.time-range-btn`, `.time-range-btn--active`.
- `.live-toggle`, `.live-dot`, `.live-dot--active`.
- `.top-list`, `.top-list-item`, `.top-list-rank`.
- `.filter-select`.

## 3. File Change Summary

### Backend (Rust)

| File | Action |
|------|--------|
| `crates/core/src/request_log.rs` | **Rewrite** -- LogStore trait + query/stats types |
| `crates/core/src/memory_log_store.rs` | **New** -- InMemoryLogStore implementation |
| `crates/core/src/file_audit.rs` | **New** -- FileAuditWriter (extracted from audit.rs) |
| `crates/core/src/audit.rs` | **Delete** |
| `crates/core/src/config.rs` | **Modify** -- AuditConfig → LogStoreConfig |
| `crates/core/src/lib.rs` | **Modify** -- update module exports |
| `crates/server/src/lib.rs` | **Modify** -- AppState + router |
| `crates/server/src/handler/dashboard/logs.rs` | **Rewrite** -- new handlers |
| `crates/server/src/handler/dashboard/websocket.rs` | **Modify** -- log_store.subscribe() |
| `crates/server/src/telemetry/gateway_log_layer.rs` | **Modify** -- use LogStore |
| `crates/server/src/dispatch.rs` | **Modify** -- config.log_store.* |
| `crates/server/src/dispatch/streaming.rs` | **Modify** -- config.log_store.* |
| `src/main.rs` | **Modify** -- build LogStore |
| `src/app.rs` | **Modify** -- accept LogStore |

### Frontend (TypeScript)

| File | Action |
|------|--------|
| `web/src/types/index.ts` | **Modify** -- extended types |
| `web/src/services/api.ts` | **Modify** -- new API methods |
| `web/src/stores/metricsStore.ts` | **Rewrite** -- simplified |
| `web/src/stores/logsStore.ts` | **Rewrite** -- drawer/filter state |
| `web/src/pages/Overview.tsx` | **Delete** |
| `web/src/pages/Metrics.tsx` | **Delete** |
| `web/src/pages/Dashboard.tsx` | **New** |
| `web/src/pages/RequestLogs.tsx` | **Rewrite** |
| `web/src/components/LogDrawer.tsx` | **New** |
| `web/src/components/JsonViewer.tsx` | **New** |
| `web/src/components/TimeRangePicker.tsx` | **New** |
| `web/src/components/FilterSelect.tsx` | **New** |
| `web/src/components/TopList.tsx` | **New** |
| `web/src/components/Layout.tsx` | **Modify** |
| `web/src/App.tsx` | **Modify** |
| `web/src/App.css` | **Modify** |
| `web/src/hooks/useWebSocket.ts` | **Modify** |

## 4. Implementation Order

1. Backend trait + InMemoryLogStore + FileAuditWriter + config
2. Backend handlers + router wiring
3. Backend integration (GatewayLogLayer, dispatch, main.rs, app.rs)
4. Backend tests
5. Frontend types + API + stores
6. Frontend components (JsonViewer, TimeRangePicker, FilterSelect, TopList, LogDrawer)
7. Frontend pages (Dashboard, RequestLogs)
8. Frontend layout + routing
9. Frontend styles
10. End-to-end verification
