use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;
use tokio::sync::Mutex;

/// Trait: pluggable audit backend (file/database/remote service etc.).
#[async_trait]
pub trait AuditBackend: Send + Sync {
    async fn write(&self, entry: &AuditEntry);
    async fn flush(&self);
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditEntry {
    pub timestamp: DateTime<Utc>,
    pub request_id: String,
    pub method: String,
    pub path: String,
    pub status: u16,
    pub latency_ms: u64,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cost: Option<f64>,
    pub error: Option<String>,
    pub api_key_id: Option<String>,
    pub tenant_id: Option<String>,
    pub client_ip: Option<String>,
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
    async fn write(&self, entry: &AuditEntry) {
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
    async fn write(&self, _entry: &AuditEntry) {}
    async fn flush(&self) {}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct AuditConfig {
    pub enabled: bool,
    pub dir: String,
    pub retention_days: u32,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            dir: "./logs/audit".to_string(),
            retention_days: 30,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_noop_backend() {
        let backend = NoopAuditBackend;
        let entry = AuditEntry {
            timestamp: Utc::now(),
            request_id: "test-123".to_string(),
            method: "POST".to_string(),
            path: "/v1/chat/completions".to_string(),
            status: 200,
            latency_ms: 100,
            provider: Some("openai".to_string()),
            model: Some("gpt-4".to_string()),
            input_tokens: Some(10),
            output_tokens: Some(20),
            cost: Some(0.001),
            error: None,
            api_key_id: None,
            tenant_id: None,
            client_ip: None,
        };
        backend.write(&entry).await;
        backend.flush().await;
    }

    #[tokio::test]
    async fn test_file_backend_writes() {
        let dir = tempfile::tempdir().unwrap();
        let config = AuditConfig {
            enabled: true,
            dir: dir.path().to_string_lossy().to_string(),
            retention_days: 30,
        };
        let backend = FileAuditBackend::new(config).unwrap();

        let entry = AuditEntry {
            timestamp: Utc::now(),
            request_id: "test-456".to_string(),
            method: "POST".to_string(),
            path: "/v1/messages".to_string(),
            status: 200,
            latency_ms: 250,
            provider: Some("claude".to_string()),
            model: Some("claude-3-opus".to_string()),
            input_tokens: Some(100),
            output_tokens: Some(200),
            cost: Some(0.01),
            error: None,
            api_key_id: Some("sk-p****1234".to_string()),
            tenant_id: Some("alpha".to_string()),
            client_ip: Some("1.2.3.4".to_string()),
        };
        backend.write(&entry).await;
        backend.flush().await;

        // Check that the file was created
        let today = Utc::now().date_naive();
        let filename = format!("audit-{}.jsonl", today.format("%Y-%m-%d"));
        let path = dir.path().join(filename);
        assert!(path.exists());

        let content = std::fs::read_to_string(path).unwrap();
        assert!(content.contains("test-456"));
    }
}
