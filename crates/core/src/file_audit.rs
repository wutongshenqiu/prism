use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;
use tokio::sync::Mutex;

use crate::request_record::RequestRecord;

const DATE_FORMAT: &str = "%Y-%m-%d";

/// Configuration for file-based audit logging.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct FileAuditConfig {
    pub enabled: bool,
    pub dir: String,
    pub retention_days: u32,
}

impl Default for FileAuditConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            dir: "./logs/audit".to_string(),
            retention_days: 30,
        }
    }
}

/// Internal state protected by a single mutex: current date + writer.
struct WriterState {
    date: NaiveDate,
    writer: Option<std::io::BufWriter<std::fs::File>>,
}

/// Append-only JSONL file writer with daily rotation.
pub struct FileAuditWriter {
    dir: String,
    state: Mutex<WriterState>,
}

impl FileAuditWriter {
    pub fn new(config: &FileAuditConfig) -> std::io::Result<Self> {
        std::fs::create_dir_all(&config.dir)?;
        let today = Utc::now().date_naive();
        let writer = Self::open_writer(&config.dir, today)?;
        Ok(Self {
            dir: config.dir.clone(),
            state: Mutex::new(WriterState {
                date: today,
                writer: Some(writer),
            }),
        })
    }

    fn open_writer(
        dir: &str,
        date: NaiveDate,
    ) -> std::io::Result<std::io::BufWriter<std::fs::File>> {
        let filename = format!("audit-{}.jsonl", date.format(DATE_FORMAT));
        let path = Path::new(dir).join(filename);
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Ok(std::io::BufWriter::new(file))
    }

    /// Write a record to the audit file. Uses a single lock for both
    /// date-rotation check and the actual write.
    pub async fn write(&self, entry: &RequestRecord) {
        let json = match serde_json::to_string(entry) {
            Ok(j) => j,
            Err(e) => {
                tracing::warn!("Failed to serialize audit entry: {e}");
                return;
            }
        };

        let mut state = self.state.lock().await;

        // Rotate if the date has changed
        let today = Utc::now().date_naive();
        if state.date != today
            && let Ok(new_writer) = Self::open_writer(&self.dir, today)
        {
            state.writer = Some(new_writer);
            state.date = today;
        }

        if let Some(ref mut w) = state.writer
            && let Err(e) = writeln!(w, "{json}")
        {
            tracing::warn!("Failed to write audit entry: {e}");
        }
    }

    /// Spawn a background task that removes old audit files daily.
    /// The first cleanup is deferred by one full interval.
    pub fn spawn_cleanup_static(dir: String, retention_days: u32) {
        tokio::spawn(async move {
            let period = std::time::Duration::from_secs(86400);
            let mut interval =
                tokio::time::interval_at(tokio::time::Instant::now() + period, period);
            loop {
                interval.tick().await;
                Self::cleanup_old_files(&dir, retention_days);
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
                    && let Ok(date) = NaiveDate::parse_from_str(date_str, DATE_FORMAT)
                    && date < cutoff
                {
                    let _ = std::fs::remove_file(entry.path());
                    tracing::info!("Removed old audit file: {name}");
                }
            }
        }
    }
}
