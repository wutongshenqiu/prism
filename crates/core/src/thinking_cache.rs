use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

// ─── Config ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct ThinkingCacheConfig {
    pub enabled: bool,
    pub ttl_secs: u64,
    pub max_entries: u64,
}

impl Default for ThinkingCacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ttl_secs: 10800, // 3 hours
            max_entries: 50_000,
        }
    }
}

// ─── Cache key ─────────────────────────────────────────────────────────────

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
struct ThinkingCacheKey {
    tenant_id: String,
    model_group: String,
    content_hash: [u8; 32],
}

impl ThinkingCacheKey {
    fn new(tenant_id: &str, model_group: &str, thinking_text: &str) -> Self {
        let content_hash: [u8; 32] = sha2::Sha256::digest(thinking_text.as_bytes()).into();
        Self {
            tenant_id: tenant_id.to_string(),
            model_group: model_group.to_string(),
            content_hash,
        }
    }
}

// ─── ThinkingCache ─────────────────────────────────────────────────────────

pub struct ThinkingCache {
    cache: moka::future::Cache<ThinkingCacheKey, String>,
    hits: AtomicU64,
    misses: AtomicU64,
    inserts: AtomicU64,
}

pub struct ThinkingCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub inserts: u64,
    pub entries: u64,
    pub hit_rate: f64,
}

impl ThinkingCache {
    pub fn new(config: &ThinkingCacheConfig) -> Self {
        let cache = moka::future::Cache::builder()
            .max_capacity(config.max_entries)
            .time_to_live(Duration::from_secs(config.ttl_secs))
            .build();
        Self {
            cache,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            inserts: AtomicU64::new(0),
        }
    }

    /// Store a thinking text → signature mapping.
    pub async fn insert(&self, tenant_id: &str, model: &str, thinking_text: &str, signature: &str) {
        let model_group = extract_model_group(model);
        let key = ThinkingCacheKey::new(tenant_id, &model_group, thinking_text);
        self.cache.insert(key, signature.to_string()).await;
        self.inserts.fetch_add(1, Ordering::Relaxed);
    }

    /// Look up a cached signature for the given thinking text.
    pub async fn get(&self, tenant_id: &str, model: &str, thinking_text: &str) -> Option<String> {
        let model_group = extract_model_group(model);
        let key = ThinkingCacheKey::new(tenant_id, &model_group, thinking_text);
        match self.cache.get(&key).await {
            Some(sig) => {
                self.hits.fetch_add(1, Ordering::Relaxed);
                Some(sig)
            }
            None => {
                self.misses.fetch_add(1, Ordering::Relaxed);
                None
            }
        }
    }

    pub fn stats(&self) -> ThinkingCacheStats {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;
        ThinkingCacheStats {
            hits,
            misses,
            inserts: self.inserts.load(Ordering::Relaxed),
            entries: self.cache.entry_count(),
            hit_rate: if total > 0 {
                hits as f64 / total as f64
            } else {
                0.0
            },
        }
    }

    /// Extract thinking signatures from a Claude response body and cache them.
    pub async fn extract_from_response(&self, tenant_id: &str, model: &str, response_body: &[u8]) {
        let Ok(resp) = serde_json::from_slice::<serde_json::Value>(response_body) else {
            return;
        };
        let Some(content) = resp.get("content").and_then(|c| c.as_array()) else {
            return;
        };

        for block in content {
            let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if block_type != "thinking" {
                continue;
            }
            let Some(thinking_text) = block.get("thinking").and_then(|t| t.as_str()) else {
                continue;
            };
            let Some(signature) = block.get("signature").and_then(|s| s.as_str()) else {
                continue;
            };
            if !thinking_text.is_empty() && !signature.is_empty() {
                self.insert(tenant_id, model, thinking_text, signature)
                    .await;
            }
        }
    }

    /// Extract thinking signatures from a streaming SSE event and cache them.
    /// Handles `content_block_start` events with `type: "thinking"` that include
    /// both `thinking` and `signature` fields.
    pub async fn extract_from_stream_event(
        &self,
        tenant_id: &str,
        model: &str,
        event_type: Option<&str>,
        data: &[u8],
    ) {
        // In streaming, complete thinking blocks with signatures appear at content_block_stop
        // but the thinking text is accumulated across content_block_delta events.
        // For signature caching in streams, we rely on the non-stream response path
        // or post-stream reconstruction. The primary use case is:
        // 1. Client sends multi-turn request with previous thinking blocks
        // 2. We inject cached signatures before sending upstream
        // So the cache is primarily populated from non-stream responses.
        // For stream events, we check content_block_start for pre-populated thinking blocks.
        if event_type != Some("content_block_start") {
            return;
        }
        let Ok(event) = serde_json::from_slice::<serde_json::Value>(data) else {
            return;
        };
        let Some(cb) = event.get("content_block") else {
            return;
        };
        let block_type = cb.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if block_type != "thinking" {
            return;
        }
        let Some(thinking_text) = cb.get("thinking").and_then(|t| t.as_str()) else {
            return;
        };
        let Some(signature) = cb.get("signature").and_then(|s| s.as_str()) else {
            return;
        };
        if !thinking_text.is_empty() && !signature.is_empty() {
            self.insert(tenant_id, model, thinking_text, signature)
                .await;
        }
    }

    /// Inject cached signatures into a Claude request body.
    /// Looks through messages for assistant thinking blocks that have `thinking`
    /// text but no `signature`, and injects cached signatures when available.
    pub async fn inject_into_request(
        &self,
        tenant_id: &str,
        model: &str,
        body: &mut serde_json::Value,
    ) -> u32 {
        let mut injected = 0u32;
        let Some(messages) = body.get_mut("messages").and_then(|m| m.as_array_mut()) else {
            return 0;
        };

        for msg in messages.iter_mut() {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
            if role != "assistant" {
                continue;
            }
            let Some(content) = msg.get_mut("content").and_then(|c| c.as_array_mut()) else {
                continue;
            };
            for block in content.iter_mut() {
                let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                if block_type != "thinking" {
                    continue;
                }
                let Some(thinking_text) = block.get("thinking").and_then(|t| t.as_str()) else {
                    continue;
                };
                if thinking_text.is_empty() {
                    continue;
                }
                // Check if signature is missing or empty
                let has_signature = block
                    .get("signature")
                    .and_then(|s| s.as_str())
                    .is_some_and(|s| !s.is_empty());
                if has_signature {
                    continue;
                }
                // Try to inject cached signature
                if let Some(sig) = self.get(tenant_id, model, thinking_text).await {
                    block["signature"] = serde_json::Value::String(sig);
                    injected += 1;
                    tracing::debug!(
                        "Injected cached thinking signature for model={model} tenant={tenant_id}"
                    );
                }
            }
        }
        injected
    }
}

/// Extract model family/group from a model name.
/// e.g., "claude-sonnet-4-5-20250514" → "claude-sonnet-4-5"
/// This allows signature reuse across minor model version changes.
fn extract_model_group(model: &str) -> String {
    // Strip date suffix like -20250514 or -20241022
    let stripped = if model.len() > 9 {
        let suffix = &model[model.len() - 9..];
        if suffix.starts_with('-')
            && suffix[1..].chars().all(|c| c.is_ascii_digit())
            && suffix.len() == 9
        {
            &model[..model.len() - 9]
        } else {
            model
        }
    } else {
        model
    };
    stripped.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_model_group() {
        assert_eq!(
            extract_model_group("claude-sonnet-4-5-20250514"),
            "claude-sonnet-4-5"
        );
        assert_eq!(
            extract_model_group("claude-3-5-sonnet-20241022"),
            "claude-3-5-sonnet"
        );
        assert_eq!(
            extract_model_group("claude-sonnet-4-5"),
            "claude-sonnet-4-5"
        );
        assert_eq!(extract_model_group("gpt-4"), "gpt-4");
    }

    #[test]
    fn test_thinking_cache_key_deterministic() {
        let k1 = ThinkingCacheKey::new("tenant-1", "claude-sonnet", "thinking text");
        let k2 = ThinkingCacheKey::new("tenant-1", "claude-sonnet", "thinking text");
        assert_eq!(k1, k2);
    }

    #[test]
    fn test_thinking_cache_key_tenant_isolation() {
        let k1 = ThinkingCacheKey::new("tenant-1", "claude-sonnet", "thinking text");
        let k2 = ThinkingCacheKey::new("tenant-2", "claude-sonnet", "thinking text");
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_thinking_cache_key_model_isolation() {
        let k1 = ThinkingCacheKey::new("tenant-1", "claude-sonnet", "thinking text");
        let k2 = ThinkingCacheKey::new("tenant-1", "claude-opus", "thinking text");
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_default_config() {
        let config = ThinkingCacheConfig::default();
        assert!(config.enabled);
        assert_eq!(config.ttl_secs, 10800);
        assert_eq!(config.max_entries, 50_000);
    }

    #[tokio::test]
    async fn test_cache_insert_and_get() {
        let config = ThinkingCacheConfig::default();
        let cache = ThinkingCache::new(&config);

        cache
            .insert("t1", "claude-sonnet-4-5", "my thinking", "sig123")
            .await;

        let result = cache.get("t1", "claude-sonnet-4-5", "my thinking").await;
        assert_eq!(result, Some("sig123".to_string()));

        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.inserts, 1);
    }

    #[tokio::test]
    async fn test_cache_miss() {
        let config = ThinkingCacheConfig::default();
        let cache = ThinkingCache::new(&config);

        let result = cache.get("t1", "claude-sonnet-4-5", "nonexistent").await;
        assert_eq!(result, None);

        let stats = cache.stats();
        assert_eq!(stats.misses, 1);
    }

    #[tokio::test]
    async fn test_cache_model_group_normalization() {
        let config = ThinkingCacheConfig::default();
        let cache = ThinkingCache::new(&config);

        // Insert with dated model
        cache
            .insert("t1", "claude-sonnet-4-5-20250514", "thinking", "sig")
            .await;

        // Retrieve with different date
        let result = cache
            .get("t1", "claude-sonnet-4-5-20250601", "thinking")
            .await;
        assert_eq!(result, Some("sig".to_string()));

        // Retrieve with base model name
        let result = cache.get("t1", "claude-sonnet-4-5", "thinking").await;
        assert_eq!(result, Some("sig".to_string()));
    }

    #[tokio::test]
    async fn test_extract_from_response() {
        let config = ThinkingCacheConfig::default();
        let cache = ThinkingCache::new(&config);

        let response = json!({
            "id": "msg_123",
            "model": "claude-sonnet-4-5-20250514",
            "content": [
                {
                    "type": "thinking",
                    "thinking": "Let me think about this...",
                    "signature": "sig_abc123"
                },
                {
                    "type": "text",
                    "text": "Here is my answer."
                }
            ]
        });
        let data = serde_json::to_vec(&response).unwrap();

        cache
            .extract_from_response("t1", "claude-sonnet-4-5-20250514", &data)
            .await;

        let result = cache
            .get("t1", "claude-sonnet-4-5", "Let me think about this...")
            .await;
        assert_eq!(result, Some("sig_abc123".to_string()));
    }

    #[tokio::test]
    async fn test_extract_from_response_no_thinking() {
        let config = ThinkingCacheConfig::default();
        let cache = ThinkingCache::new(&config);

        let response = json!({
            "id": "msg_123",
            "model": "claude-sonnet-4-5",
            "content": [
                {"type": "text", "text": "Hello"}
            ]
        });
        let data = serde_json::to_vec(&response).unwrap();

        cache
            .extract_from_response("t1", "claude-sonnet-4-5", &data)
            .await;

        assert_eq!(cache.stats().inserts, 0);
    }

    #[tokio::test]
    async fn test_inject_into_request() {
        let config = ThinkingCacheConfig::default();
        let cache = ThinkingCache::new(&config);

        // Pre-populate cache
        cache
            .insert("t1", "claude-sonnet-4-5", "previous thinking", "cached_sig")
            .await;

        // Request with thinking block missing signature
        let mut body = json!({
            "model": "claude-sonnet-4-5",
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "thinking",
                            "thinking": "previous thinking"
                        },
                        {
                            "type": "text",
                            "text": "previous answer"
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": "follow-up question"
                }
            ]
        });

        let injected = cache
            .inject_into_request("t1", "claude-sonnet-4-5", &mut body)
            .await;
        assert_eq!(injected, 1);

        // Verify signature was injected
        let sig = body["messages"][0]["content"][0]["signature"]
            .as_str()
            .unwrap();
        assert_eq!(sig, "cached_sig");
    }

    #[tokio::test]
    async fn test_inject_skips_existing_signature() {
        let config = ThinkingCacheConfig::default();
        let cache = ThinkingCache::new(&config);

        cache
            .insert("t1", "claude-sonnet-4-5", "thinking", "cached")
            .await;

        let mut body = json!({
            "model": "claude-sonnet-4-5",
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "thinking",
                            "thinking": "thinking",
                            "signature": "original_sig"
                        }
                    ]
                }
            ]
        });

        let injected = cache
            .inject_into_request("t1", "claude-sonnet-4-5", &mut body)
            .await;
        assert_eq!(injected, 0);

        // Original signature preserved
        assert_eq!(
            body["messages"][0]["content"][0]["signature"],
            "original_sig"
        );
    }

    #[tokio::test]
    async fn test_inject_no_messages() {
        let config = ThinkingCacheConfig::default();
        let cache = ThinkingCache::new(&config);

        let mut body = json!({"model": "claude-sonnet-4-5"});
        let injected = cache
            .inject_into_request("t1", "claude-sonnet-4-5", &mut body)
            .await;
        assert_eq!(injected, 0);
    }
}
