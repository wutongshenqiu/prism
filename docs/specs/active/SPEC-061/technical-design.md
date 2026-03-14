# Technical Design: Quota-Aware Credential Switching

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-061       |
| Title     | Quota-Aware Credential Switching |
| Author    | AI Agent       |
| Status    | Draft          |
| Created   | 2026-03-14     |
| Updated   | 2026-03-14     |

## Overview

Adds quota-aware cooldown to credential routing. When a 429 response is received, the credential enters a temporary cooldown period and is skipped during selection until the cooldown expires.

## Backend Implementation

### Key Changes

```rust
// In crates/provider/src/routing.rs
struct QuotaCooldown {
    until: Instant,
}

// Add to CredentialRouter
cooldowns: DashMap<CredentialId, QuotaCooldown>,
```

### Flow

1. Request fails with 429
2. Extract `Retry-After` header value (or use default 60s)
3. Set cooldown: `cooldowns.insert(cred_id, QuotaCooldown { until: now + duration })`
4. During credential selection: skip credentials where `now < cooldown.until`
5. Cooldown expires naturally

## Configuration Changes

```yaml
quota-cooldown-default-secs: 60
```

## Task Breakdown

- [ ] Add QuotaCooldown to CredentialRouter
- [ ] Record 429 cooldown in dispatch
- [ ] Check cooldown during credential selection
- [ ] Add config option
- [ ] Unit tests
- [ ] Integration tests

## Test Strategy

- **Unit tests:** Cooldown set/check/expire logic
- **Integration tests:** Mock 429 response, verify credential skip
