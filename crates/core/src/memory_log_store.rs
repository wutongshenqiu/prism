use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::RwLock;

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use tokio::sync::broadcast;

use crate::file_audit::FileAuditWriter;
use crate::request_log::*;
use crate::request_record::{RequestRecord, TokenUsage};

#[derive(Default)]
struct TimeBucket {
    requests: u64,
    errors: u64,
    latency_sum: u64,
    tokens: u64,
    cost: f64,
}

#[derive(Default)]
struct ModelAccum {
    requests: u64,
    latency_sum: u64,
    tokens: u64,
    cost: f64,
}

/// In-memory ring buffer implementation of [`LogStore`].
pub struct InMemoryLogStore {
    entries: RwLock<VecDeque<RequestRecord>>,
    capacity: usize,
    tx: broadcast::Sender<RequestRecord>,
    file_writer: Option<FileAuditWriter>,
}

fn field_contains(field: Option<&str>, needle: &str) -> bool {
    field.is_some_and(|v| v.to_lowercase().contains(needle))
}

impl InMemoryLogStore {
    pub fn new(capacity: usize, file_writer: Option<FileAuditWriter>) -> Self {
        let (tx, _) = broadcast::channel(256);
        Self {
            entries: RwLock::new(VecDeque::with_capacity(capacity)),
            capacity,
            tx,
            file_writer,
        }
    }

    /// Check if a record matches all filters in the query.
    /// `keyword_lower` is a pre-lowercased keyword to avoid repeated allocation.
    fn matches(e: &RequestRecord, q: &LogQuery, keyword_lower: Option<&str>) -> bool {
        if let Some(ref id) = q.request_id
            && e.request_id != *id
        {
            return false;
        }
        if let Some(ref t) = q.tenant_id
            && e.tenant_id.as_deref() != Some(t.as_str())
        {
            return false;
        }
        if let Some(ref k) = q.api_key_id
            && e.api_key_id.as_deref() != Some(k.as_str())
        {
            return false;
        }
        if let Some(ref p) = q.provider
            && e.provider.as_deref() != Some(p.as_str())
        {
            return false;
        }
        if let Some(ref m) = q.model
            && e.model.as_deref() != Some(m.as_str())
        {
            return false;
        }
        if let Some(ref s) = q.status {
            let ok = match s.as_str() {
                "2xx" => (200..300).contains(&e.status),
                "4xx" => (400..500).contains(&e.status),
                "5xx" => (500..600).contains(&e.status),
                other => other.parse::<u16>().is_ok_and(|code| e.status == code),
            };
            if !ok {
                return false;
            }
        }
        if let Some(ref et) = q.error_type
            && e.error_type.as_deref() != Some(et.as_str())
        {
            return false;
        }
        if let Some(s) = q.stream
            && e.stream != s
        {
            return false;
        }
        let ts = e.timestamp.timestamp_millis();
        if let Some(from) = q.from
            && ts < from
        {
            return false;
        }
        if let Some(to) = q.to
            && ts > to
        {
            return false;
        }
        if let Some(min) = q.latency_min
            && e.latency_ms < min
        {
            return false;
        }
        if let Some(max) = q.latency_max
            && e.latency_ms > max
        {
            return false;
        }
        if let Some(kw) = keyword_lower {
            let found = field_contains(e.request_body.as_deref(), kw)
                || field_contains(e.upstream_request_body.as_deref(), kw)
                || field_contains(e.response_body.as_deref(), kw)
                || field_contains(e.stream_content_preview.as_deref(), kw)
                || field_contains(e.error.as_deref(), kw);
            if !found {
                return false;
            }
        }
        true
    }

    /// Determine time series bucket interval in seconds based on the query time range.
    fn bucket_interval_secs(from: Option<i64>, to: Option<i64>) -> i64 {
        let range_ms = match (from, to) {
            (Some(f), Some(t)) => t - f,
            _ => 3_600_000, // default 1h
        };
        let range_secs = range_ms / 1000;
        if range_secs <= 900 {
            10 // ≤ 15m → 10s buckets
        } else if range_secs <= 3600 {
            60 // ≤ 1h → 1m buckets
        } else if range_secs <= 21600 {
            300 // ≤ 6h → 5m buckets
        } else if range_secs <= 86400 {
            900 // ≤ 24h → 15m buckets
        } else {
            3600 // > 24h → 1h buckets
        }
    }

    fn compute_percentile(sorted: &[u64], p: f64) -> u64 {
        if sorted.is_empty() {
            return 0;
        }
        let idx = ((p / 100.0) * sorted.len() as f64).ceil() as usize;
        sorted[idx.min(sorted.len()) - 1]
    }
}

#[async_trait]
impl LogStore for InMemoryLogStore {
    async fn push(&self, entry: RequestRecord) {
        // Only clone for broadcast if there are active subscribers
        if self.tx.receiver_count() > 0 {
            let _ = self.tx.send(entry.clone());
        }

        // Write to file audit if enabled
        if let Some(ref writer) = self.file_writer {
            writer.write(&entry).await;
        }

        if let Ok(mut entries) = self.entries.write() {
            if entries.len() >= self.capacity {
                entries.pop_front();
            }
            entries.push_back(entry);
        }
    }

    async fn get(&self, request_id: &str) -> Option<RequestRecord> {
        let entries = self.entries.read().unwrap();
        entries
            .iter()
            .rfind(|e| e.request_id == request_id)
            .cloned()
    }

    async fn query(&self, q: &LogQuery) -> LogPage {
        let page = q.page.unwrap_or(1).max(1);
        let page_size = q.page_size.unwrap_or(50).clamp(1, 200);

        // Pre-compute keyword lowercase once outside the per-record loop
        let keyword_lower = q.keyword.as_ref().map(|kw| kw.to_lowercase());
        let keyword_ref = keyword_lower.as_deref();

        let has_custom_sort = q.sort_by.is_some();
        let start = (page - 1) * page_size;

        if has_custom_sort {
            // Custom sort requires collecting all matches, sorting, then paginating.
            let mut filtered: Vec<RequestRecord> = {
                let entries = self.entries.read().unwrap();
                entries
                    .iter()
                    .rev()
                    .filter(|e| Self::matches(e, q, keyword_ref))
                    .cloned()
                    .collect()
            };

            let sort_by = q.sort_by.as_ref().unwrap();
            let desc = !matches!(q.sort_order, Some(SortOrder::Asc));
            filtered.sort_by(|a, b| {
                let cmp = match sort_by {
                    SortField::Timestamp => a.timestamp.cmp(&b.timestamp),
                    SortField::Latency => a.latency_ms.cmp(&b.latency_ms),
                    SortField::Cost => a
                        .cost
                        .unwrap_or(0.0)
                        .partial_cmp(&b.cost.unwrap_or(0.0))
                        .unwrap_or(std::cmp::Ordering::Equal),
                };
                if desc { cmp.reverse() } else { cmp }
            });

            let total = filtered.len();
            let total_pages = if total == 0 {
                0
            } else {
                total.div_ceil(page_size)
            };
            let data: Vec<RequestRecord> =
                filtered.into_iter().skip(start).take(page_size).collect();

            return LogPage {
                data,
                total,
                page,
                page_size,
                total_pages,
            };
        }

        // Default order (newest first) — count total, then clone only the page slice.
        let (total, data) = {
            let entries = self.entries.read().unwrap();
            let matching: Vec<&RequestRecord> = entries
                .iter()
                .rev()
                .filter(|e| Self::matches(e, q, keyword_ref))
                .collect();
            let total = matching.len();
            let data: Vec<RequestRecord> = matching
                .into_iter()
                .skip(start)
                .take(page_size)
                .cloned()
                .collect();
            (total, data)
        };

        let total_pages = if total == 0 {
            0
        } else {
            total.div_ceil(page_size)
        };

        LogPage {
            data,
            total,
            page,
            page_size,
            total_pages,
        }
    }

    async fn stats(&self, q: &StatsQuery) -> LogStats {
        // Reuse `matches` by converting StatsQuery → LogQuery
        let lq = LogQuery {
            from: q.from,
            to: q.to,
            provider: q.provider.clone(),
            model: q.model.clone(),
            ..Default::default()
        };

        let bucket_secs = Self::bucket_interval_secs(q.from, q.to);

        // Single-pass aggregation under read lock — no cloning needed.
        // All &str keys borrow from entries held by the read guard.
        let entries = self.entries.read().unwrap();

        let mut total = 0usize;
        let mut errors = 0u64;
        let mut latency_sum = 0u64;
        let mut latencies: Vec<u64> = Vec::new();
        let mut total_cost = 0.0f64;
        let mut total_tokens = 0u64;
        let mut buckets: BTreeMap<i64, TimeBucket> = BTreeMap::new();
        let mut model_map: HashMap<&str, ModelAccum> = HashMap::new();
        let mut error_map: HashMap<&str, (u64, DateTime<Utc>)> = HashMap::new();
        let mut prov_map: HashMap<&str, u64> = HashMap::new();
        let mut status_dist = StatusDistribution::default();

        for e in entries.iter().filter(|e| Self::matches(e, &lq, None)) {
            total += 1;
            // Latency
            latencies.push(e.latency_ms);
            latency_sum += e.latency_ms;

            // Error count + status distribution
            let is_error = e.status >= 400;
            if is_error {
                errors += 1;
            }
            match e.status {
                200..300 => status_dist.success += 1,
                400..500 => status_dist.client_error += 1,
                500..600 => status_dist.server_error += 1,
                _ => {}
            }

            // Cost & tokens
            if let Some(c) = e.cost {
                total_cost += c;
            }
            let entry_tokens = e.usage.as_ref().map_or(0, |u| u.total());
            if entry_tokens > 0 {
                total_tokens += entry_tokens;
            }

            // Time series bucket
            let ts = e.timestamp.timestamp();
            let bucket_key = ts - (ts % bucket_secs);
            let bucket = buckets.entry(bucket_key).or_default();
            bucket.requests += 1;
            if is_error {
                bucket.errors += 1;
            }
            bucket.latency_sum += e.latency_ms;
            bucket.tokens += entry_tokens;
            if let Some(c) = e.cost {
                bucket.cost += c;
            }

            // Model stats
            if let Some(ref m) = e.model {
                let ms = model_map.entry(m.as_str()).or_default();
                ms.requests += 1;
                ms.latency_sum += e.latency_ms;
                ms.tokens += entry_tokens;
                if let Some(c) = e.cost {
                    ms.cost += c;
                }
            }

            // Error type stats
            if let Some(ref et) = e.error_type {
                let es = error_map
                    .entry(et.as_str())
                    .or_insert((0, DateTime::UNIX_EPOCH));
                es.0 += 1;
                if e.timestamp > es.1 {
                    es.1 = e.timestamp;
                }
            }

            // Provider distribution
            if let Some(ref p) = e.provider {
                *prov_map.entry(p.as_str()).or_default() += 1;
            }
        }

        // Percentiles (requires sorted latencies)
        latencies.sort_unstable();
        let avg_latency = if total > 0 {
            latency_sum / total as u64
        } else {
            0
        };
        let p50 = Self::compute_percentile(&latencies, 50.0);
        let p95 = Self::compute_percentile(&latencies, 95.0);
        let p99 = Self::compute_percentile(&latencies, 99.0);

        // Build time series
        let time_series: Vec<TimeSeriesBucket> = buckets
            .into_iter()
            .map(|(ts, b)| {
                let dt: DateTime<Utc> = Utc.timestamp_opt(ts, 0).unwrap();
                TimeSeriesBucket {
                    timestamp: dt.to_rfc3339(),
                    requests: b.requests,
                    errors: b.errors,
                    avg_latency_ms: if b.requests > 0 {
                        b.latency_sum / b.requests
                    } else {
                        0
                    },
                    tokens: b.tokens,
                    cost: b.cost,
                }
            })
            .collect();

        // Build top models
        let mut top_models: Vec<ModelStats> = model_map
            .into_iter()
            .map(|(m, a)| ModelStats {
                model: m.to_string(),
                requests: a.requests,
                avg_latency_ms: if a.requests > 0 {
                    a.latency_sum / a.requests
                } else {
                    0
                },
                total_tokens: a.tokens,
                total_cost: a.cost,
            })
            .collect();
        top_models.sort_by(|a, b| b.requests.cmp(&a.requests));
        top_models.truncate(10);

        // Build top errors
        let mut top_errors: Vec<ErrorStats> = error_map
            .into_iter()
            .map(|(et, (count, last))| ErrorStats {
                error_type: et.to_string(),
                count,
                last_seen: last.to_rfc3339(),
            })
            .collect();
        top_errors.sort_by(|a, b| b.count.cmp(&a.count));
        top_errors.truncate(10);

        // Build provider distribution
        let total_f = total as f64;
        let mut provider_distribution: Vec<ProviderDistribution> = prov_map
            .into_iter()
            .map(|(p, count)| ProviderDistribution {
                provider: p.to_string(),
                requests: count,
                percentage: if total_f > 0.0 {
                    (count as f64 / total_f) * 100.0
                } else {
                    0.0
                },
            })
            .collect();
        provider_distribution.sort_by(|a, b| b.requests.cmp(&a.requests));

        // All &str borrows from entries have been consumed; release the read lock.
        drop(entries);

        LogStats {
            total_entries: total,
            error_count: errors as usize,
            avg_latency_ms: avg_latency,
            p50_latency_ms: p50,
            p95_latency_ms: p95,
            p99_latency_ms: p99,
            total_cost,
            total_tokens,
            time_series,
            top_models,
            top_errors,
            provider_distribution,
            status_distribution: status_dist,
        }
    }

    async fn filter_options(&self) -> FilterOptions {
        // Use &str sets to avoid cloning every string; only allocate at the end
        let entries = self.entries.read().unwrap();
        let mut providers: HashSet<&str> = HashSet::new();
        let mut models: HashSet<&str> = HashSet::new();
        let mut error_types: HashSet<&str> = HashSet::new();
        let mut tenant_ids: HashSet<&str> = HashSet::new();

        for e in entries.iter() {
            if let Some(ref p) = e.provider {
                providers.insert(p.as_str());
            }
            if let Some(ref m) = e.model {
                models.insert(m.as_str());
            }
            if let Some(ref et) = e.error_type {
                error_types.insert(et.as_str());
            }
            if let Some(ref t) = e.tenant_id {
                tenant_ids.insert(t.as_str());
            }
        }

        let mut providers: Vec<String> = providers.into_iter().map(String::from).collect();
        providers.sort();
        let mut models: Vec<String> = models.into_iter().map(String::from).collect();
        models.sort();
        let mut error_types: Vec<String> = error_types.into_iter().map(String::from).collect();
        error_types.sort();
        let mut tenant_ids: Vec<String> = tenant_ids.into_iter().map(String::from).collect();
        tenant_ids.sort();

        FilterOptions {
            providers,
            models,
            error_types,
            tenant_ids,
        }
    }

    fn subscribe(&self) -> broadcast::Receiver<RequestRecord> {
        self.tx.subscribe()
    }

    async fn update_usage(&self, request_id: &str, usage: TokenUsage, cost: Option<f64>) {
        if let Ok(mut entries) = self.entries.write()
            && let Some(entry) = entries.iter_mut().rfind(|e| e.request_id == request_id)
        {
            entry.usage = Some(usage);
            entry.cost = cost;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_entry(status: u16, provider: &str, model: &str) -> RequestRecord {
        RequestRecord {
            request_id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            method: "POST".to_string(),
            path: "/v1/chat/completions".to_string(),
            stream: false,
            requested_model: Some(model.to_string()),
            request_body: None,
            upstream_request_body: None,
            provider: Some(provider.to_string()),
            model: Some(model.to_string()),
            credential_name: None,
            total_attempts: 1,
            status,
            latency_ms: 100,
            response_body: None,
            stream_content_preview: None,
            usage: Some(crate::request_record::TokenUsage {
                input_tokens: 10,
                output_tokens: 20,
                ..Default::default()
            }),
            cost: Some(0.001),
            error: if status >= 400 {
                Some("error".to_string())
            } else {
                None
            },
            error_type: if status >= 500 {
                Some("upstream_5xx".to_string())
            } else if status == 429 {
                Some("rate_limited".to_string())
            } else {
                None
            },
            api_key_id: None,
            tenant_id: None,
            client_ip: None,
            client_region: None,
            attempts: vec![],
        }
    }

    #[tokio::test]
    async fn test_push_and_query() {
        let store = InMemoryLogStore::new(100, None);
        for i in 0..10 {
            let status = if i % 3 == 0 { 500 } else { 200 };
            store.push(make_entry(status, "openai", "gpt-4")).await;
        }

        let page = store.query(&LogQuery::default()).await;
        assert_eq!(page.total, 10);
        assert_eq!(page.page, 1);
    }

    #[tokio::test]
    async fn test_capacity_eviction() {
        let store = InMemoryLogStore::new(5, None);
        for _ in 0..10 {
            store.push(make_entry(200, "openai", "gpt-4")).await;
        }
        let page = store.query(&LogQuery::default()).await;
        assert_eq!(page.total, 5);
    }

    #[tokio::test]
    async fn test_get_by_id() {
        let store = InMemoryLogStore::new(100, None);
        let entry = make_entry(200, "openai", "gpt-4");
        let id = entry.request_id.clone();
        store.push(entry).await;

        assert!(store.get(&id).await.is_some());
        assert!(store.get("nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn test_filter_by_provider() {
        let store = InMemoryLogStore::new(100, None);
        store.push(make_entry(200, "openai", "gpt-4")).await;
        store.push(make_entry(200, "claude", "claude-3")).await;
        store.push(make_entry(200, "openai", "gpt-3.5")).await;

        let page = store
            .query(&LogQuery {
                provider: Some("openai".to_string()),
                ..Default::default()
            })
            .await;
        assert_eq!(page.total, 2);
    }

    #[tokio::test]
    async fn test_filter_by_status() {
        let store = InMemoryLogStore::new(100, None);
        store.push(make_entry(200, "openai", "gpt-4")).await;
        store.push(make_entry(429, "openai", "gpt-4")).await;
        store.push(make_entry(500, "openai", "gpt-4")).await;

        let page = store
            .query(&LogQuery {
                status: Some("4xx".to_string()),
                ..Default::default()
            })
            .await;
        assert_eq!(page.total, 1);

        let page = store
            .query(&LogQuery {
                status: Some("5xx".to_string()),
                ..Default::default()
            })
            .await;
        assert_eq!(page.total, 1);
    }

    #[tokio::test]
    async fn test_pagination() {
        let store = InMemoryLogStore::new(100, None);
        for _ in 0..25 {
            store.push(make_entry(200, "openai", "gpt-4")).await;
        }

        let page = store
            .query(&LogQuery {
                page: Some(2),
                page_size: Some(10),
                ..Default::default()
            })
            .await;
        assert_eq!(page.total, 25);
        assert_eq!(page.data.len(), 10);
        assert_eq!(page.page, 2);
    }

    #[tokio::test]
    async fn test_stats() {
        let store = InMemoryLogStore::new(100, None);
        store.push(make_entry(200, "openai", "gpt-4")).await;
        store.push(make_entry(500, "openai", "gpt-4")).await;
        store.push(make_entry(200, "claude", "claude-3")).await;

        let stats = store.stats(&StatsQuery::default()).await;
        assert_eq!(stats.total_entries, 3);
        assert_eq!(stats.error_count, 1);
        assert_eq!(stats.provider_distribution.len(), 2);
        assert!(!stats.top_models.is_empty());
    }

    #[tokio::test]
    async fn test_filter_options() {
        let store = InMemoryLogStore::new(100, None);
        store.push(make_entry(200, "openai", "gpt-4")).await;
        store.push(make_entry(500, "claude", "claude-3")).await;

        let opts = store.filter_options().await;
        assert_eq!(opts.providers.len(), 2);
        assert_eq!(opts.models.len(), 2);
        assert_eq!(opts.error_types.len(), 1); // upstream_5xx from 500
    }

    #[tokio::test]
    async fn test_update_usage() {
        let store = InMemoryLogStore::new(100, None);
        let entry = make_entry(200, "openai", "gpt-4");
        let id = entry.request_id.clone();
        store.push(entry).await;

        let new_usage = TokenUsage {
            input_tokens: 999,
            output_tokens: 888,
            ..Default::default()
        };
        store.update_usage(&id, new_usage, Some(1.23)).await;

        let record = store.get(&id).await.unwrap();
        assert_eq!(record.usage.unwrap().input_tokens, 999);
        assert_eq!(record.cost, Some(1.23));
    }

    #[tokio::test]
    async fn test_keyword_search() {
        let store = InMemoryLogStore::new(100, None);
        let mut entry = make_entry(200, "openai", "gpt-4");
        entry.request_body = Some(r#"{"messages":[{"content":"hello world"}]}"#.to_string());
        store.push(entry).await;

        store.push(make_entry(200, "openai", "gpt-4")).await;

        let page = store
            .query(&LogQuery {
                keyword: Some("hello".to_string()),
                ..Default::default()
            })
            .await;
        assert_eq!(page.total, 1);
    }

    #[tokio::test]
    async fn test_sort_by_latency() {
        let store = InMemoryLogStore::new(100, None);
        let mut e1 = make_entry(200, "openai", "gpt-4");
        e1.latency_ms = 100;
        let mut e2 = make_entry(200, "openai", "gpt-4");
        e2.latency_ms = 500;
        let mut e3 = make_entry(200, "openai", "gpt-4");
        e3.latency_ms = 50;
        store.push(e1).await;
        store.push(e2).await;
        store.push(e3).await;

        let page = store
            .query(&LogQuery {
                sort_by: Some(SortField::Latency),
                sort_order: Some(SortOrder::Desc),
                ..Default::default()
            })
            .await;
        assert_eq!(page.data[0].latency_ms, 500);
        assert_eq!(page.data[2].latency_ms, 50);
    }
}
