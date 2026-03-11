use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::request_record::{RequestRecord, TokenUsage};

// ── Sort ──

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

// ── Query ──

#[derive(Debug, Default, Deserialize)]
pub struct LogQuery {
    pub page: Option<usize>,
    pub page_size: Option<usize>,

    // Exact match
    pub request_id: Option<String>,
    pub tenant_id: Option<String>,
    pub api_key_id: Option<String>,

    // Filter
    pub provider: Option<String>,
    pub model: Option<String>,
    /// "2xx", "4xx", "5xx", or a specific status code like "429".
    pub status: Option<String>,
    pub error_type: Option<String>,
    pub stream: Option<bool>,

    // Range
    pub from: Option<i64>,
    pub to: Option<i64>,
    pub latency_min: Option<u64>,
    pub latency_max: Option<u64>,

    // Keyword substring search across body/error fields.
    pub keyword: Option<String>,

    // Sort
    pub sort_by: Option<SortField>,
    pub sort_order: Option<SortOrder>,
}

// ── Paged response ──

#[derive(Debug, Serialize)]
pub struct LogPage {
    pub data: Vec<RequestRecord>,
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
    pub total_pages: usize,
}

// ── Stats ──

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
    pub timestamp: String,
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

#[derive(Debug, Default, Serialize)]
pub struct StatusDistribution {
    pub success: u64,
    pub client_error: u64,
    pub server_error: u64,
}

// ── Filter options ──

#[derive(Debug, Default, Serialize)]
pub struct FilterOptions {
    pub providers: Vec<String>,
    pub models: Vec<String>,
    pub error_types: Vec<String>,
    pub tenant_ids: Vec<String>,
}

// ── Trait ──

#[async_trait]
pub trait LogStore: Send + Sync {
    /// Store a new log entry.
    async fn push(&self, entry: RequestRecord);

    /// Retrieve a single record by request ID.
    async fn get(&self, request_id: &str) -> Option<RequestRecord>;

    /// Paginated query with filtering and sorting.
    async fn query(&self, q: &LogQuery) -> LogPage;

    /// Aggregated statistics over a time range.
    async fn stats(&self, q: &StatsQuery) -> LogStats;

    /// Distinct values available for filter dropdowns.
    async fn filter_options(&self) -> FilterOptions;

    /// Subscribe to new log entries (for WebSocket fanout).
    fn subscribe(&self) -> broadcast::Receiver<RequestRecord>;

    /// Update usage and cost for a streaming request after completion.
    async fn update_usage(&self, request_id: &str, usage: TokenUsage, cost: Option<f64>);
}
