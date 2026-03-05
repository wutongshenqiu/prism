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
    let uptime_secs = state.start_time.elapsed().as_secs();

    (
        StatusCode::OK,
        Json(json!({
            "status": "ok",
            "version": env!("CARGO_PKG_VERSION"),
            "uptime_secs": uptime_secs,
            "host": config.host,
            "port": config.port,
            "tls_enabled": config.tls.enable,
            "providers": {
                "claude": config.claude_api_key.iter().filter(|k| !k.disabled).count(),
                "openai": config.openai_api_key.iter().filter(|k| !k.disabled).count(),
                "gemini": config.gemini_api_key.iter().filter(|k| !k.disabled).count(),
                "openai_compat": config.openai_compatibility.iter().filter(|k| !k.disabled).count(),
            },
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
    let contents = match read_file_tail(&file_path, MAX_READ_BYTES) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "read_failed", "message": e.to_string()})),
            );
        }
    };

    let mut lines: Vec<&str> = contents.lines().rev().collect();

    // Filter by level
    if let Some(ref level) = query.level {
        let level_upper = level.to_uppercase();
        lines.retain(|l| l.to_uppercase().contains(&level_upper));
    }

    // Filter by search
    if let Some(ref search) = query.search {
        lines.retain(|l| l.contains(search.as_str()));
    }

    let total = lines.len();
    let start = (query.page - 1) * query.page_size;
    let page_lines: Vec<&str> = lines
        .into_iter()
        .skip(start)
        .take(query.page_size)
        .collect();

    (
        StatusCode::OK,
        Json(json!({
            "logs": page_lines,
            "total": total,
            "page": query.page,
            "page_size": query.page_size,
            "file": file_path.display().to_string(),
        })),
    )
}
