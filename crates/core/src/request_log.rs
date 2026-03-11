use serde::Deserialize;
use std::collections::VecDeque;
use std::sync::RwLock;
use tokio::sync::broadcast;

use crate::request_record::RequestRecord;

/// Query parameters for filtering request logs.
#[derive(Debug, Default, Deserialize)]
pub struct LogQuery {
    pub page: Option<usize>,
    pub page_size: Option<usize>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub status: Option<String>,
    pub from: Option<i64>,
    pub to: Option<i64>,
}

/// Paged response for log queries.
#[derive(Debug, serde::Serialize)]
pub struct LogPage {
    pub items: Vec<RequestRecord>,
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
}

/// In-memory ring buffer for request logs with broadcast notification.
pub struct RequestLogStore {
    entries: RwLock<VecDeque<RequestRecord>>,
    capacity: usize,
    tx: broadcast::Sender<RequestRecord>,
}

impl RequestLogStore {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(256);
        Self {
            entries: RwLock::new(VecDeque::with_capacity(capacity)),
            capacity,
            tx,
        }
    }

    /// Push a new log entry. Evicts the oldest if at capacity.
    pub fn push(&self, entry: RequestRecord) {
        let _ = self.tx.send(entry.clone());
        if let Ok(mut entries) = self.entries.write() {
            if entries.len() >= self.capacity {
                entries.pop_front();
            }
            entries.push_back(entry);
        }
    }

    /// Subscribe to new log entries.
    pub fn subscribe(&self) -> broadcast::Receiver<RequestRecord> {
        self.tx.subscribe()
    }

    /// Query logs with filtering and pagination.
    pub fn query(&self, q: &LogQuery) -> LogPage {
        let page = q.page.unwrap_or(1).max(1);
        let page_size = q.page_size.unwrap_or(50).clamp(1, 200);

        let entries = self.entries.read().unwrap();
        let filtered: Vec<&RequestRecord> = entries
            .iter()
            .rev() // newest first
            .filter(|e| {
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
                    let matches = match s.as_str() {
                        "2xx" => (200..300).contains(&e.status),
                        "4xx" => (400..500).contains(&e.status),
                        "5xx" => (500..600).contains(&e.status),
                        other => {
                            if let Ok(code) = other.parse::<u16>() {
                                e.status == code
                            } else {
                                true
                            }
                        }
                    };
                    if !matches {
                        return false;
                    }
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
                true
            })
            .collect();

        let total = filtered.len();
        let start = (page - 1) * page_size;
        let items: Vec<RequestRecord> = filtered
            .into_iter()
            .skip(start)
            .take(page_size)
            .cloned()
            .collect();

        LogPage {
            items,
            total,
            page,
            page_size,
        }
    }

    /// Return summary statistics.
    pub fn stats(&self) -> serde_json::Value {
        let entries = self.entries.read().unwrap();
        let total = entries.len();
        let errors = entries.iter().filter(|e| e.status >= 400).count();
        let avg_latency = if total > 0 {
            entries.iter().map(|e| e.latency_ms).sum::<u64>() / total as u64
        } else {
            0
        };
        serde_json::json!({
            "total_entries": total,
            "capacity": self.capacity,
            "error_count": errors,
            "avg_latency_ms": avg_latency,
        })
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
            provider: Some(provider.to_string()),
            model: Some(model.to_string()),
            credential_name: None,
            retry_count: 0,
            status,
            latency_ms: 100,
            usage: Some(crate::request_record::TokenUsage {
                input_tokens: 10,
                output_tokens: 20,
                ..Default::default()
            }),
            cost: None,
            error: if status >= 400 {
                Some("error".to_string())
            } else {
                None
            },
            api_key_id: None,
            tenant_id: None,
            client_ip: None,
        }
    }

    #[test]
    fn test_push_and_query() {
        let store = RequestLogStore::new(100);
        for i in 0..10 {
            let status = if i % 3 == 0 { 500 } else { 200 };
            store.push(make_entry(status, "openai", "gpt-4"));
        }

        let page = store.query(&LogQuery::default());
        assert_eq!(page.total, 10);
        assert_eq!(page.page, 1);
    }

    #[test]
    fn test_capacity_eviction() {
        let store = RequestLogStore::new(5);
        for _ in 0..10 {
            store.push(make_entry(200, "openai", "gpt-4"));
        }
        let page = store.query(&LogQuery::default());
        assert_eq!(page.total, 5);
    }

    #[test]
    fn test_filter_by_provider() {
        let store = RequestLogStore::new(100);
        store.push(make_entry(200, "openai", "gpt-4"));
        store.push(make_entry(200, "claude", "claude-3"));
        store.push(make_entry(200, "openai", "gpt-3.5"));

        let page = store.query(&LogQuery {
            provider: Some("openai".to_string()),
            ..Default::default()
        });
        assert_eq!(page.total, 2);
    }

    #[test]
    fn test_filter_by_status() {
        let store = RequestLogStore::new(100);
        store.push(make_entry(200, "openai", "gpt-4"));
        store.push(make_entry(429, "openai", "gpt-4"));
        store.push(make_entry(500, "openai", "gpt-4"));

        let page = store.query(&LogQuery {
            status: Some("4xx".to_string()),
            ..Default::default()
        });
        assert_eq!(page.total, 1);

        let page = store.query(&LogQuery {
            status: Some("5xx".to_string()),
            ..Default::default()
        });
        assert_eq!(page.total, 1);
    }

    #[test]
    fn test_pagination() {
        let store = RequestLogStore::new(100);
        for _ in 0..25 {
            store.push(make_entry(200, "openai", "gpt-4"));
        }

        let page = store.query(&LogQuery {
            page: Some(2),
            page_size: Some(10),
            ..Default::default()
        });
        assert_eq!(page.total, 25);
        assert_eq!(page.items.len(), 10);
        assert_eq!(page.page, 2);
    }

    #[test]
    fn test_stats() {
        let store = RequestLogStore::new(100);
        store.push(make_entry(200, "openai", "gpt-4"));
        store.push(make_entry(500, "openai", "gpt-4"));

        let stats = store.stats();
        assert_eq!(stats["total_entries"], 2);
        assert_eq!(stats["error_count"], 1);
    }
}
