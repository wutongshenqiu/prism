use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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

    // ── Routing ──
    /// Provider format name (e.g., "openai", "claude", "gemini", "openai-compat").
    pub provider: Option<String>,
    /// Actual model name used after routing/fallback.
    pub model: Option<String>,
    /// Name of the credential selected by the router.
    pub credential_name: Option<String>,
    /// Number of retry attempts before success (0 = first attempt succeeded).
    #[serde(default)]
    pub retry_count: u32,

    // ── Response ──
    pub status: u16,
    pub latency_ms: u64,

    // ── Usage & Cost ──
    pub usage: Option<TokenUsage>,
    pub cost: Option<f64>,

    // ── Error ──
    pub error: Option<String>,

    // ── Client ──
    /// Masked API key (first 4 + last 4 chars).
    pub api_key_id: Option<String>,
    pub tenant_id: Option<String>,
    pub client_ip: Option<String>,
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
            provider: Some("openai".to_string()),
            model: Some("gpt-4".to_string()),
            credential_name: Some("prod-key".to_string()),
            retry_count: 1,
            status: 200,
            latency_ms: 150,
            usage: Some(TokenUsage {
                input_tokens: 100,
                output_tokens: 50,
                cache_read_tokens: 200,
                cache_creation_tokens: 0,
            }),
            cost: Some(0.0035),
            error: None,
            api_key_id: Some("sk-p****1234".to_string()),
            tenant_id: Some("alpha".to_string()),
            client_ip: Some("1.2.3.4".to_string()),
        };
        let json = serde_json::to_string(&record).unwrap();
        let deserialized: RequestRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.request_id, "req-123");
        assert_eq!(deserialized.retry_count, 1);
        assert!(deserialized.usage.is_some());
        assert_eq!(deserialized.usage.unwrap().cache_read_tokens, 200);
    }
}
