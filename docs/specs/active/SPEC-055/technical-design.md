# Technical Design: Gemini Native API Endpoints

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-055       |
| Title     | Gemini Native API Endpoints |
| Author    | AI Agent       |
| Status    | Draft          |
| Created   | 2026-03-14     |
| Updated   | 2026-03-14     |

## Overview

Adds Gemini-native HTTP endpoints to Prism, enabling it to serve as a drop-in proxy for Gemini CLI and SDK clients. Includes reverse translation (Gemini→OpenAI request) for cross-provider routing.

## API Design

### Endpoints

```
POST /v1beta/models/{model}:generateContent
POST /v1beta/models/{model}:streamGenerateContent
GET  /v1beta/models
POST /v1internal:generateContent
POST /v1internal:streamGenerateContent
```

### Authentication

- `x-goog-api-key` header
- `?key=...` query parameter
- Maps to existing auth key store

### Request (Gemini native format)

```json
{
  "contents": [{"role": "user", "parts": [{"text": "Hello"}]}],
  "generationConfig": {"temperature": 0.7}
}
```

### Response (Gemini native format)

```json
{
  "candidates": [{"content": {"role": "model", "parts": [{"text": "Hi!"}]}}],
  "usageMetadata": {"promptTokenCount": 5, "candidatesTokenCount": 10}
}
```

## Backend Implementation

### Module Structure

```
crates/server/src/handler/
└── gemini.rs           # Gemini native endpoint handlers

crates/translator/src/
├── gemini_to_openai_request.rs    # Gemini → OpenAI request translation
└── openai_to_gemini_response.rs   # OpenAI → Gemini response translation
```

### Key Types

```rust
// Handler extracts model from URL path
async fn generate_content(
    State(state): State<AppState>,
    Path(model): Path<String>,
    body: Bytes,
) -> Result<Response, ProxyError>
```

### Flow

1. Client sends Gemini-format request to `/v1beta/models/{model}:generateContent`
2. Handler extracts model from URL path, sets `source_format = Format::Gemini`
3. Auth validated via `x-goog-api-key` header or `?key=` param
4. Dispatch routes to best provider
5. If target is Gemini: passthrough
6. If target is OpenAI/Claude: translate Gemini→OpenAI request, execute, translate response back

## Configuration Changes

No new configuration. Uses existing provider and auth key configuration.

## Provider Compatibility

| Provider | Supported | Notes |
|----------|-----------|-------|
| Gemini   | Yes       | Native passthrough |
| OpenAI   | Yes       | Via Gemini→OpenAI→OpenAI translation |
| Claude   | Yes       | Via Gemini→OpenAI→Claude translation chain |

## Task Breakdown

- [ ] Create gemini.rs handler with generateContent endpoint
- [ ] Create streamGenerateContent endpoint
- [ ] Create models listing endpoint (Gemini format)
- [ ] Create v1internal endpoints with localhost restriction
- [ ] Create gemini_to_openai_request.rs translator
- [ ] Create openai_to_gemini_response.rs translator
- [ ] Register routes in router
- [ ] Register translators in registry
- [ ] Add unit tests for translators
- [ ] Add integration tests for endpoints

## Test Strategy

- **Unit tests:** Gemini→OpenAI request translation, OpenAI→Gemini response translation
- **Integration tests:** Full request cycle through Gemini endpoints
- **Manual verification:** Gemini CLI connection test

## Rollout Plan

1. Implement handlers and translators
2. Register routes
3. Test with Gemini CLI
