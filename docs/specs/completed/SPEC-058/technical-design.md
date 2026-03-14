# Technical Design: Provider-Scoped Routing & Amp Integration

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-058       |
| Title     | Provider-Scoped Routing & Amp Integration |
| Author    | AI Agent       |
| Status    | Draft          |
| Created   | 2026-03-14     |
| Updated   | 2026-03-14     |

## Overview

Adds provider-scoped API endpoints that allow clients to specify which provider should handle their request. Reuses existing dispatch infrastructure with an additional credential filter.

## API Design

### Endpoints

```
POST /api/provider/{provider}/v1/chat/completions
POST /api/provider/{provider}/v1/messages
POST /api/provider/{provider}/v1/responses
```

### Provider Resolution

1. Match `{provider}` against credential names (exact match)
2. Fall back to format name match (openai, claude, gemini, openai-compat)
3. Return 404 if no match

## Backend Implementation

### Handler

```rust
// crates/server/src/handler/provider_scoped.rs
async fn provider_scoped_chat(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError>
```

### Flow

1. Extract `{provider}` from path
2. Resolve to credential set
3. Call dispatch with `allowed_credentials` filter
4. Return response

## Task Breakdown

- [ ] Create provider_scoped.rs handler
- [ ] Add provider resolution logic
- [ ] Register routes
- [ ] Add dispatch support for allowed_credentials filter
- [ ] Unit tests
- [ ] Integration tests

## Test Strategy

- **Unit tests:** Provider resolution logic
- **Integration tests:** Full request with provider-scoped routing
- **Manual verification:** Amp CLI connection test
