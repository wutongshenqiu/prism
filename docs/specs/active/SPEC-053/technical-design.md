# Technical Design: Thinking Signature Cache

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-053       |
| Title     | Thinking Signature Cache |
| Author    | AI Agent       |
| Status    | Draft          |
| Created   | 2026-03-14     |
| Updated   | 2026-03-14     |

## Overview

Implements an in-memory cache that maps thinking block content to their signatures. During response processing, thinking+signature pairs are extracted and cached. During request processing, thinking blocks without signatures have cached signatures injected. This ensures multi-turn conversations work correctly even when routed to different credentials.

## API Design

No new API endpoints. This is an internal optimization transparent to clients.

### Configuration

```yaml
thinking-cache:
  enabled: true
  ttl-secs: 10800      # 3 hours
  max-entries: 50000
```

## Backend Implementation

### Module Structure

```
crates/core/src/
└── thinking_cache.rs    # ThinkingCache struct + ThinkingCacheConfig
```

### Key Types

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct ThinkingCacheConfig {
    pub enabled: bool,
    pub ttl_secs: u64,
    pub max_entries: u64,
}

pub struct ThinkingCache {
    cache: moka::future::Cache<ThinkingCacheKey, String>,
}

#[derive(Hash, Eq, PartialEq, Clone)]
struct ThinkingCacheKey {
    tenant_id: String,
    model_group: String,
    content_hash: [u8; 32],  // SHA256
}
```

### Flow

1. **Response path** (after receiving Claude response):
   - Parse response body for `content` array
   - Find blocks with `type: "thinking"` that have both `thinking` text and `signature`
   - Compute SHA256 of thinking text
   - Store `(tenant_id, model_group, hash) → signature` in cache

2. **Request path** (before sending to Claude):
   - Parse request body `messages` array
   - Find thinking blocks in assistant messages that have `thinking` text but no `signature`
   - Look up signature in cache by `(tenant_id, model_group, SHA256(thinking_text))`
   - If found, inject the signature

## Configuration Changes

Add to `Config` struct in `crates/core/src/config.rs`:
```rust
pub thinking_cache: ThinkingCacheConfig,
```

## Provider Compatibility

| Provider | Supported | Notes |
|----------|-----------|-------|
| Claude   | Yes       | Primary target — thinking blocks with signatures |
| OpenAI   | N/A       | No thinking signature concept |
| Gemini   | N/A       | No thinking signature concept |

## Task Breakdown

- [ ] Create `ThinkingCacheConfig` in config.rs
- [ ] Create `thinking_cache.rs` with `ThinkingCache` struct
- [ ] Add `ThinkingCache` to AppState
- [ ] Implement signature extraction from Claude responses
- [ ] Implement signature injection into Claude requests
- [ ] Add unit tests for cache operations
- [ ] Add integration test for multi-turn thinking conversation

## Test Strategy

- **Unit tests:** Cache insert/lookup/eviction, key computation, config parsing
- **Integration tests:** Mock multi-turn conversation with thinking blocks, verify signature injection
- **Manual verification:** Test with real Claude thinking model through proxy

## Rollout Plan

1. Add ThinkingCacheConfig with `enabled: false` default
2. Implement cache and integration
3. Enable by default after testing
