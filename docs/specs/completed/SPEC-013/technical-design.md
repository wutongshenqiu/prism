# Technical Design: Model Fallback & Debug Mode

| Field     | Value                       |
|-----------|-----------------------------|
| Spec ID   | SPEC-013                    |
| Title     | Model Fallback & Debug Mode |
| Author    | AI Proxy Team               |
| Status    | Completed                   |
| Created   | 2026-03-01                  |
| Updated   | 2026-03-01                  |

## Overview

Adds model fallback chains and debug headers to the dispatch engine. Clients can specify a `models` array in the request body; the proxy tries each model in order until one succeeds. The `x-debug: true` request header enables debug response headers showing routing decisions.

Reference: [SPEC-013 PRD](prd.md)

## API Design

### Request — Model Fallback

```json
{
  "models": ["gpt-4o", "gpt-4o-mini", "claude-sonnet-4-6"],
  "messages": [{"role": "user", "content": "hello"}]
}
```

The first model in the array is the primary; subsequent models are fallbacks tried in order on failure.

### Request — Debug Mode

```
x-debug: true
```

### Response — Debug Headers

```
x-debug-provider: openai
x-debug-model: gpt-4o
x-debug-credential: openai-primary
x-debug-attempts: gpt-4o@openai, gpt-4o-mini@openai
```

## Backend Implementation

### Module Structure

```
crates/server/src/
├── handler/mod.rs     # ParsedRequest: models[], debug flag
└── dispatch.rs        # DispatchRequest, DispatchDebug, fallback loop

crates/core/src/
├── config.rs          # ProviderKeyEntry.weight
└── provider.rs        # AuthRecord.credential_name, weight

crates/provider/src/
└── routing.rs         # Weighted round-robin in CredentialRouter::pick()
```

### Key Types

```rust
// crates/server/src/handler/mod.rs
pub(crate) struct ParsedRequest {
    pub model: String,
    pub models: Option<Vec<String>>,  // Fallback chain
    pub stream: bool,
    pub user_agent: Option<String>,
    pub debug: bool,                   // x-debug header
}

// crates/server/src/dispatch.rs
pub struct DispatchRequest {
    pub model: String,
    pub models: Option<Vec<String>>,
    pub stream: bool,
    pub debug: bool,
    // ...
}

#[derive(Debug, Default)]
struct DispatchDebug {
    provider: Option<String>,
    model: Option<String>,
    credential_name: Option<String>,
    attempts: Vec<String>,           // "model@provider" format
}

// crates/core/src/provider.rs
pub struct AuthRecord {
    pub credential_name: Option<String>,
    pub weight: u32,                   // Default: 1
    // ...
}
```

### Flow

1. `parse_request()` extracts `models` array from body and `x-debug` from headers
2. `dispatch()` builds `model_chain`: if `models` present use it, else `[model]`
3. **Outer loop**: iterate through `model_chain`
4. **Inner loop**: retry with backoff per model (existing retry logic)
5. On each attempt, `DispatchDebug.attempts.push("model@provider")`
6. On success, set `DispatchDebug.{provider, model, credential_name}`
7. `rewrite_model_in_body()` replaces model field for each fallback attempt
8. If `debug == true`, `inject_debug_headers()` writes `x-debug-*` response headers

### Weighted Round-Robin

```rust
// crates/provider/src/routing.rs — CredentialRouter::pick()
let total_weight: u32 = candidates.iter().map(|c| c.weight.max(1)).sum();
let slot = (counter as u32) % total_weight;
let mut cumulative = 0u32;
for c in &candidates {
    cumulative += c.weight.max(1);
    if slot < cumulative {
        return Some(c.clone());
    }
}
```

## Configuration Changes

```yaml
claude-api-key:
  - api-key: "sk-ant-..."
    name: "claude-primary"
    weight: 2                # Gets 2x traffic vs weight=1
  - api-key: "sk-ant-..."
    name: "claude-secondary"
    weight: 1                # Default weight
```

## Provider Compatibility

| Provider | Supported | Notes |
|----------|-----------|-------|
| OpenAI   | Yes       | Model fallback across providers |
| Claude   | Yes       | Cross-format translation on fallback |
| Gemini   | Yes       | Cross-format translation on fallback |
| OpenAI-compat | Yes  | Model name rewriting per attempt |

## Task Breakdown

- [x] T1: `ParsedRequest` — add `models` and `debug` fields, parse from body/headers
- [x] T2: `DispatchRequest` — add `models` and `debug` fields
- [x] T3: Dispatch fallback outer loop (iterate `model_chain`)
- [x] T4: `DispatchDebug` struct + `inject_debug_headers()` for `x-debug-*` response headers
- [x] T5: `rewrite_model_in_body()` — JSON model field replacement per fallback attempt
- [x] T6: `ProviderKeyEntry.weight` + `AuthRecord.weight` + weighted round-robin in `CredentialRouter::pick()`
- [x] T7: `config.example.yaml` — document `weight` and `models` usage

## Test Strategy

- **Unit tests:** Weighted round-robin distribution, model body rewriting
- **Integration tests:** Dashboard API verifies routing config persistence
- **Manual verification:** Send request with `models` array and `x-debug: true`, verify fallback chain and debug headers
