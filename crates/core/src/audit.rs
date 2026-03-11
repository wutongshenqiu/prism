use async_trait::async_trait;
use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;
use tokio::sync::Mutex;

use crate::request_record::RequestRecord;

/// Trait: pluggable audit backend (file/database/remote service etc.).
#[async_trait]
pub trait AuditBackend: Send + Sync {
    async fn write(&self, entry: &RequestRecord);
    async fn flush(&self);
}

/// Default implementation: JSONL file with daily rotation.
pub struct FileAuditBackend {
    config: tokio::sync::RwLock<AuditConfig>,
    writer: Mutex<Option<std::io::BufWriter<std::fs::File>>>,
    current_date: Mutex<NaiveDate>,
}

impl FileAuditBackend {
    pub fn new(config: AuditConfig) -> std::io::Result<Self> {
        std::fs::create_dir_all(&config.dir)?;
        let today = Utc::now().date_naive();
        let writer = Self::open_writer(&config.dir, today)?;
        Ok(Self {
            config: tokio::sync::RwLock::new(config),
            writer: Mutex::new(Some(writer)),
            current_date: Mutex::new(today),
        })
    }

    fn open_writer(
        dir: &str,
        date: NaiveDate,
    ) -> std::io::Result<std::io::BufWriter<std::fs::File>> {
        let filename = format!("audit-{}.jsonl", date.format("%Y-%m-%d"));
        let path = Path::new(dir).join(filename);
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Ok(std::io::BufWriter::new(file))
    }

    async fn rotate_if_needed(&self) {
        let today = Utc::now().date_naive();
        let mut current = self.current_date.lock().await;
        if *current != today {
            let config = self.config.read().await;
            if let Ok(new_writer) = Self::open_writer(&config.dir, today) {
                let mut writer = self.writer.lock().await;
                *writer = Some(new_writer);
                *current = today;
            }
        }
    }

    /// Spawn a background task that removes old audit files daily.
    pub fn spawn_cleanup_task(config: AuditConfig) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(86400));
            loop {
                interval.tick().await;
                Self::cleanup_old_files(&config.dir, config.retention_days);
            }
        });
    }

    fn cleanup_old_files(dir: &str, retention_days: u32) {
        let cutoff = Utc::now().date_naive() - chrono::Duration::days(retention_days as i64);
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if let Some(date_str) = name
                    .strip_prefix("audit-")
                    .and_then(|s| s.strip_suffix(".jsonl"))
                    && let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                    && date < cutoff
                {
                    let _ = std::fs::remove_file(entry.path());
                    tracing::info!("Removed old audit file: {name}");
                }
            }
        }
    }
}

#[async_trait]
impl AuditBackend for FileAuditBackend {
    async fn write(&self, entry: &RequestRecord) {
        self.rotate_if_needed().await;
        match serde_json::to_string(entry) {
            Ok(json) => {
                let mut writer = self.writer.lock().await;
                if let Some(ref mut w) = *writer
                    && let Err(e) = writeln!(w, "{json}")
                {
                    tracing::warn!("Failed to write audit entry: {e}");
                }
            }
            Err(e) => {
                tracing::warn!("Failed to serialize audit entry: {e}");
            }
        }
    }

    async fn flush(&self) {
        let mut writer = self.writer.lock().await;
        if let Some(ref mut w) = *writer {
            let _ = w.flush();
        }
    }
}

/// Noop implementation: zero overhead when audit is disabled.
pub struct NoopAuditBackend;

#[async_trait]
impl AuditBackend for NoopAuditBackend {
    async fn write(&self, _entry: &RequestRecord) {}
    async fn flush(&self) {}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct AuditConfig {
    pub enabled: bool,
    pub dir: String,
    pub retention_days: u32,
    /// Controls how much request/response body content is captured.
    pub detail_level: crate::request_record::LogDetailLevel,
    /// Maximum bytes of body content to capture per field (request_body, response_body, etc.).
    pub max_body_bytes: usize,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            dir: "./logs/audit".to_string(),
            retention_days: 30,
            detail_level: crate::request_record::LogDetailLevel::Metadata,
            max_body_bytes: 16384,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request_record::TokenUsage;

    fn make_test_record() -> RequestRecord {
        RequestRecord {
            request_id: "test-456".to_string(),
            timestamp: Utc::now(),
            method: "POST".to_string(),
            path: "/v1/messages".to_string(),
            stream: false,
            requested_model: Some("claude-3-opus".to_string()),
            request_body: None,
            upstream_request_body: None,
            provider: Some("claude".to_string()),
            model: Some("claude-3-opus".to_string()),
            credential_name: Some("prod-key".to_string()),
            total_attempts: 1,
            status: 200,
            latency_ms: 250,
            response_body: None,
            stream_content_preview: None,
            usage: Some(TokenUsage {
                input_tokens: 100,
                output_tokens: 200,
                cache_read_tokens: 50,
                cache_creation_tokens: 0,
            }),
            cost: Some(0.01),
            error: None,
            error_type: None,
            api_key_id: Some("sk-p****1234".to_string()),
            tenant_id: Some("alpha".to_string()),
            client_ip: Some("1.2.3.4".to_string()),
            client_region: None,
            attempts: vec![],
        }
    }

    #[tokio::test]
    async fn test_noop_backend() {
        let backend = NoopAuditBackend;
        backend.write(&make_test_record()).await;
        backend.flush().await;
    }

    #[tokio::test]
    async fn test_file_backend_writes() {
        let dir = tempfile::tempdir().unwrap();
        let config = AuditConfig {
            enabled: true,
            dir: dir.path().to_string_lossy().to_string(),
            retention_days: 30,
            ..Default::default()
        };
        let backend = FileAuditBackend::new(config).unwrap();

        backend.write(&make_test_record()).await;
        backend.flush().await;

        let today = Utc::now().date_naive();
        let filename = format!("audit-{}.jsonl", today.format("%Y-%m-%d"));
        let path = dir.path().join(filename);
        assert!(path.exists());

        let content = std::fs::read_to_string(path).unwrap();
        assert!(content.contains("test-456"));
        assert!(content.contains("cache_read_tokens"));
    }
}
