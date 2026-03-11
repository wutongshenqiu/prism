use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

/// Token usage breakdown for a single request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// Cache read tokens (e.g., Claude `cache_read_input_tokens`, OpenAI `cached_tokens`).
    #[serde(default)]
    pub cache_read_tokens: u64,
    /// Cache creation tokens (e.g., Claude `cache_creation_input_tokens`).
    #[serde(default)]
    pub cache_creation_tokens: u64,
}

impl TokenUsage {
    pub fn total_input(&self) -> u64 {
        self.input_tokens + self.cache_read_tokens + self.cache_creation_tokens
    }

    pub fn total(&self) -> u64 {
        self.total_input() + self.output_tokens
    }

    /// Merge another usage into this one, taking the max of each field.
    /// Claude sends input_tokens in `message_start` and output_tokens in `message_delta`,
    /// so we accumulate by taking the max of each field across events.
    pub fn merge(&mut self, other: &TokenUsage) {
        self.input_tokens = self.input_tokens.max(other.input_tokens);
        self.output_tokens = self.output_tokens.max(other.output_tokens);
        self.cache_read_tokens = self.cache_read_tokens.max(other.cache_read_tokens);
        self.cache_creation_tokens = self.cache_creation_tokens.max(other.cache_creation_tokens);
    }
}

/// Detail level for request logging body capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum LogDetailLevel {
    /// Only metadata fields (no body content).
    #[default]
    Metadata,
    /// Metadata + truncated request/response bodies.
    Standard,
    /// Metadata + full request/response bodies (up to max_body_bytes).
    Full,
}

/// Summary of a single upstream attempt within a request's retry chain.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AttemptSummary {
    pub attempt_index: u32,
    pub provider: String,
    pub model: String,
    pub credential_name: Option<String>,
    pub status: Option<u16>,
    pub latency_ms: u64,
    pub error: Option<String>,
    pub error_type: Option<String>,
}

/// A single request record used for both in-memory log store and persistent audit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestRecord {
    // ── Identity ──
    pub request_id: String,
    pub timestamp: DateTime<Utc>,

    // ── Request ──
    pub method: String,
    pub path: String,
    pub stream: bool,
    /// The model name the client originally requested.
    pub requested_model: Option<String>,
    /// Client's original request body (before translation).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_body: Option<String>,
    /// Request body sent to upstream (after translation + cloaking + payload rules).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_request_body: Option<String>,

    // ── Routing ──
    /// Provider format name (e.g., "openai", "claude", "gemini", "openai-compat").
    pub provider: Option<String>,
    /// Actual model name used after routing/fallback.
    pub model: Option<String>,
    /// Name of the credential selected by the router.
    pub credential_name: Option<String>,
    /// Total number of upstream attempts (includes retries across providers).
    #[serde(default)]
    pub total_attempts: u32,

    // ── Response ──
    pub status: u16,
    pub latency_ms: u64,
    /// Non-stream response body (truncated per detail level).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_body: Option<String>,
    /// Stream content preview (first N chars of accumulated content).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_content_preview: Option<String>,

    // ── Usage & Cost ──
    pub usage: Option<TokenUsage>,
    pub cost: Option<f64>,

    // ── Error ──
    pub error: Option<String>,
    /// Categorized error type (e.g., "rate_limited", "upstream_5xx", "network", "timeout").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_type: Option<String>,

    // ── Client ──
    /// Masked API key (first 4 + last 4 chars).
    pub api_key_id: Option<String>,
    pub tenant_id: Option<String>,
    pub client_ip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_region: Option<String>,

    // ── Per-attempt details ──
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attempts: Vec<AttemptSummary>,
}

/// Truncate a body string to `max_bytes`, appending "...[truncated]" if truncated.
/// Returns `Cow::Borrowed` when no truncation is needed to avoid allocation.
pub fn truncate_body(body: &str, max_bytes: usize) -> Cow<'_, str> {
    if max_bytes == 0 || body.len() <= max_bytes {
        return Cow::Borrowed(body);
    }
    // Find a valid UTF-8 boundary
    let mut end = max_bytes;
    while end > 0 && !body.is_char_boundary(end) {
        end -= 1;
    }
    Cow::Owned(format!("{}...[truncated]", &body[..end]))
}

/// Classify an error into a category string for structured logging.
pub fn classify_error(error: &crate::error::ProxyError) -> &'static str {
    use crate::error::ProxyError;
    match error {
        ProxyError::Upstream { status, .. } => match *status {
            429 => "rate_limited",
            s if (500..=599).contains(&s) => "upstream_5xx",
            s if (400..=499).contains(&s) => "upstream_4xx",
            _ => "upstream_other",
        },
        ProxyError::Network(_) => "network",
        ProxyError::NoCredentials { .. } => "no_credentials",
        ProxyError::ModelCooldown { .. } | ProxyError::RateLimited { .. } => "rate_limited",
        ProxyError::Translation(_) => "translation",
        ProxyError::BadRequest(_) => "bad_request",
        _ => "internal",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_usage_totals() {
        let usage = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            cache_read_tokens: 200,
            cache_creation_tokens: 30,
        };
        assert_eq!(usage.total_input(), 330);
        assert_eq!(usage.total(), 380);
    }

    #[test]
    fn token_usage_default_is_zero() {
        let usage = TokenUsage::default();
        assert_eq!(usage.total(), 0);
    }

    #[test]
    fn token_usage_merge_takes_max() {
        let mut a = TokenUsage {
            input_tokens: 100,
            output_tokens: 0,
            cache_read_tokens: 50,
            cache_creation_tokens: 0,
        };
        let b = TokenUsage {
            input_tokens: 0,
            output_tokens: 200,
            cache_read_tokens: 0,
            cache_creation_tokens: 30,
        };
        a.merge(&b);
        assert_eq!(a.input_tokens, 100);
        assert_eq!(a.output_tokens, 200);
        assert_eq!(a.cache_read_tokens, 50);
        assert_eq!(a.cache_creation_tokens, 30);
    }

    #[test]
    fn request_record_serialization_roundtrip() {
        let record = RequestRecord {
            request_id: "req-123".to_string(),
            timestamp: Utc::now(),
            method: "POST".to_string(),
            path: "/v1/chat/completions".to_string(),
            stream: true,
            requested_model: Some("gpt-4".to_string()),
            request_body: None,
            upstream_request_body: None,
            provider: Some("openai".to_string()),
            model: Some("gpt-4".to_string()),
            credential_name: Some("prod-key".to_string()),
            total_attempts: 2,
            status: 200,
            latency_ms: 150,
            response_body: None,
            stream_content_preview: None,
            usage: Some(TokenUsage {
                input_tokens: 100,
                output_tokens: 50,
                cache_read_tokens: 200,
                cache_creation_tokens: 0,
            }),
            cost: Some(0.0035),
            error: None,
            error_type: None,
            api_key_id: Some("sk-p****1234".to_string()),
            tenant_id: Some("alpha".to_string()),
            client_ip: Some("1.2.3.4".to_string()),
            client_region: None,
            attempts: vec![],
        };
        let json = serde_json::to_string(&record).unwrap();
        let deserialized: RequestRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.request_id, "req-123");
        assert_eq!(deserialized.total_attempts, 2);
        assert!(deserialized.usage.is_some());
        assert_eq!(deserialized.usage.unwrap().cache_read_tokens, 200);
    }

    #[test]
    fn truncate_body_within_limit() {
        assert_eq!(truncate_body("hello", 10), "hello");
        assert_eq!(truncate_body("hello", 5), "hello");
    }

    #[test]
    fn truncate_body_exceeds_limit() {
        let result = truncate_body("hello world", 5);
        assert_eq!(result, "hello...[truncated]");
    }

    #[test]
    fn truncate_body_zero_limit() {
        assert_eq!(truncate_body("hello", 0), "hello");
    }

    #[test]
    fn truncate_body_utf8_boundary() {
        // Chinese characters are 3 bytes each in UTF-8
        let s = "你好世界";
        let result = truncate_body(s, 7); // 7 bytes cuts in the middle of '世'
        assert_eq!(result, "你好...[truncated]");
    }

    #[test]
    fn log_detail_level_ordering() {
        assert!(LogDetailLevel::Metadata < LogDetailLevel::Standard);
        assert!(LogDetailLevel::Standard < LogDetailLevel::Full);
    }

    #[test]
    fn attempt_summary_serialization() {
        let summary = AttemptSummary {
            attempt_index: 0,
            provider: "openai".to_string(),
            model: "gpt-4".to_string(),
            credential_name: Some("key-1".to_string()),
            status: Some(429),
            latency_ms: 50,
            error: Some("rate limited".to_string()),
            error_type: Some("rate_limited".to_string()),
        };
        let json = serde_json::to_string(&summary).unwrap();
        let de: AttemptSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(de.attempt_index, 0);
        assert_eq!(de.status, Some(429));
    }
}
