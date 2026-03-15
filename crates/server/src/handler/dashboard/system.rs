use crate::AppState;
use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;
use serde_json::json;
use std::io::{Read, Seek, SeekFrom};

/// Read the tail of a file up to `max_bytes`. Returns a String starting at
/// the first complete line within the read window.
fn read_file_tail(path: &std::path::Path, max_bytes: u64) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let metadata = file.metadata()?;
    let file_size = metadata.len();

    if file_size <= max_bytes {
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        return Ok(contents);
    }

    // Seek to (file_size - max_bytes) and skip the first partial line
    file.seek(SeekFrom::End(-(max_bytes as i64)))?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    // Skip the first partial line
    if let Some(pos) = contents.find('\n') {
        contents = contents[pos + 1..].to_string();
    }

    Ok(contents)
}

/// GET /api/dashboard/system/health
pub async fn system_health(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.load();
    let uptime_seconds = state.start_time.elapsed().as_secs();
    let health_snap = state.health_manager.snapshot();

    // Group credentials by provider name and derive runtime health.
    // A credential is "active" if it is not disabled AND not circuit-broken/ejected.
    let mut provider_groups: std::collections::HashMap<&str, (usize, usize)> =
        std::collections::HashMap::new();
    for entry in &config.providers {
        let (active, total) = provider_groups.entry(&entry.name).or_insert((0, 0));
        *total += 1;
        if !entry.disabled {
            // Check runtime health: look up by credential name
            let runtime_unhealthy = health_snap
                .credentials
                .get(&entry.name)
                .is_some_and(|h| h.circuit_open || h.ejected);
            if !runtime_unhealthy {
                *active += 1;
            }
        }
    }
    let providers: Vec<serde_json::Value> = provider_groups
        .into_iter()
        .map(|(name, (active, total))| {
            let status = if total == 0 {
                "unconfigured"
            } else if active == 0 {
                "unhealthy"
            } else if active < total {
                "degraded"
            } else {
                "healthy"
            };
            json!({
                "name": name,
                "status": status,
                "active_keys": active,
                "total_keys": total,
            })
        })
        .collect();

    // Determine overall status from provider statuses
    let has_any_provider = providers.iter().any(|p| p["status"] != "unconfigured");
    let all_healthy = providers
        .iter()
        .filter(|p| p["status"] != "unconfigured")
        .all(|p| p["status"] == "healthy");
    let any_healthy = providers
        .iter()
        .any(|p| p["status"] == "healthy" || p["status"] == "degraded");
    let status = if !has_any_provider {
        "not_configured"
    } else if all_healthy {
        "healthy"
    } else if any_healthy {
        "degraded"
    } else {
        "unhealthy"
    };

    // Collect metrics summary
    let metrics = state.metrics.snapshot();
    let metrics_summary = json!({
        "total_requests": metrics["total_requests"],
        "total_errors": metrics["total_errors"],
        "error_rate": metrics["error_rate"],
        "avg_latency_ms": metrics["avg_latency_ms"],
        "rpm": metrics["requests_per_minute"],
        "total_tokens": metrics["total_tokens"],
        "total_cost_usd": metrics["total_cost_usd"],
        "cache_hits": metrics["cache"]["hits"],
        "cache_misses": metrics["cache"]["misses"],
    });

    (
        StatusCode::OK,
        Json(json!({
            "status": status,
            "version": env!("CARGO_PKG_VERSION"),
            "uptime_seconds": uptime_seconds,
            "host": config.host,
            "port": config.port,
            "tls_enabled": config.tls.enable,
            "providers": providers,
            "metrics": metrics_summary,
        })),
    )
}

#[derive(Debug, Deserialize)]
pub struct LogsQuery {
    #[serde(default = "default_page")]
    pub page: usize,
    #[serde(default = "default_page_size")]
    pub page_size: usize,
    pub level: Option<String>,
    pub search: Option<String>,
}

fn default_page() -> usize {
    1
}
fn default_page_size() -> usize {
    100
}

/// GET /api/dashboard/system/logs
pub async fn system_logs(
    State(state): State<AppState>,
    Query(query): Query<LogsQuery>,
) -> impl IntoResponse {
    let config = state.config.load();
    let log_dir = config.log_dir.as_deref().unwrap_or("./logs");

    let log_path = std::path::Path::new(log_dir);
    if !log_path.exists() {
        return (
            StatusCode::OK,
            Json(json!({
                "logs": [],
                "total": 0,
                "message": "Log directory not found or logging to file not enabled"
            })),
        );
    }

    // Find the most recent log file
    let mut log_files: Vec<_> = match std::fs::read_dir(log_path) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|ext| ext == "log" || ext == "json")
            })
            .collect(),
        Err(_) => {
            return (StatusCode::OK, Json(json!({"logs": [], "total": 0})));
        }
    };

    log_files.sort_by_key(|f| std::cmp::Reverse(f.metadata().and_then(|m| m.modified()).ok()));

    let file_path = match log_files.first() {
        Some(f) => f.path(),
        None => {
            return (StatusCode::OK, Json(json!({"logs": [], "total": 0})));
        }
    };

    // Read only the tail of the log file to avoid OOM on large files.
    // We read the last 2MB which is sufficient for recent log viewing.
    const MAX_READ_BYTES: u64 = 2 * 1024 * 1024;
    let file_size = file_path.metadata().map(|m| m.len()).unwrap_or(0);
    let truncated = file_size > MAX_READ_BYTES;
    let contents = match read_file_tail(&file_path, MAX_READ_BYTES) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "read_failed", "message": e.to_string()})),
            );
        }
    };

    // Parse log lines into structured entries
    let parsed: Vec<serde_json::Value> = contents
        .lines()
        .rev()
        .map(|line| {
            // Try to parse as JSON structured log (tracing-subscriber JSON format)
            if let Ok(obj) = serde_json::from_str::<serde_json::Value>(line) {
                json!({
                    "timestamp": obj.get("timestamp").or_else(|| obj.get("ts")).and_then(|v| v.as_str()).unwrap_or(""),
                    "level": obj.get("level").and_then(|v| v.as_str()).unwrap_or("INFO").to_uppercase(),
                    "target": obj.get("target").or_else(|| obj.get("module")).and_then(|v| v.as_str()).unwrap_or(""),
                    "message": obj.get("fields").and_then(|f| f.get("message")).or_else(|| obj.get("message")).and_then(|v| v.as_str()).unwrap_or(line),
                })
            } else {
                // Fallback: try to extract level from raw log line
                let level = if line.contains("ERROR") {
                    "ERROR"
                } else if line.contains("WARN") {
                    "WARN"
                } else if line.contains("DEBUG") {
                    "DEBUG"
                } else if line.contains("TRACE") {
                    "TRACE"
                } else {
                    "INFO"
                };
                json!({
                    "timestamp": "",
                    "level": level,
                    "target": "",
                    "message": line,
                })
            }
        })
        .collect();

    // Filter by level
    let filtered: Vec<&serde_json::Value> = parsed
        .iter()
        .filter(|entry| {
            if let Some(ref level) = query.level {
                let level_upper = level.to_uppercase();
                entry["level"].as_str().is_some_and(|l| l == level_upper)
            } else {
                true
            }
        })
        .filter(|entry| {
            if let Some(ref search) = query.search {
                let msg = entry["message"].as_str().unwrap_or("");
                let target = entry["target"].as_str().unwrap_or("");
                msg.contains(search.as_str()) || target.contains(search.as_str())
            } else {
                true
            }
        })
        .collect();

    let total = filtered.len();
    let start = (query.page - 1) * query.page_size;
    let page_entries: Vec<&serde_json::Value> = filtered
        .into_iter()
        .skip(start)
        .take(query.page_size)
        .collect();

    (
        StatusCode::OK,
        Json(json!({
            "logs": page_entries,
            "total": total,
            "page": query.page,
            "page_size": query.page_size,
            "file": file_path.display().to_string(),
            "truncated": truncated,
        })),
    )
}
