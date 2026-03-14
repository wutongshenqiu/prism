# Technical Design: Vertex AI Provider

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-062       |
| Title     | Vertex AI Provider |
| Author    | AI Agent       |
| Status    | Draft          |
| Created   | 2026-03-14     |
| Updated   | 2026-03-14     |

## Overview

Adds a Vertex AI executor that constructs Vertex-specific URLs and uses OAuth2 Bearer authentication while reusing the Gemini format and all existing Gemini translators.

## Backend Implementation

### Key Types

```rust
// crates/provider/src/vertex.rs
pub struct VertexAIExecutor {
    client: reqwest::Client,
}

// Vertex AI URL pattern
fn build_vertex_url(region: &str, project: &str, location: &str, model: &str) -> String {
    format!(
        "https://{region}-aiplatform.googleapis.com/v1/projects/{project}/locations/{location}/publishers/google/models/{model}:generateContent"
    )
}
```

### Configuration

```yaml
gemini-api-key:
  - name: vertex-prod
    vertex: true
    project: my-project
    location: us-central1
    service-account: /path/to/sa-key.json
```

## Provider Compatibility

| Provider | Supported | Notes |
|----------|-----------|-------|
| Vertex AI | Yes     | Primary target |
| Gemini   | N/A     | Uses same Format::Gemini |

## Task Breakdown

- [ ] Create vertex.rs executor
- [ ] Implement URL construction
- [ ] Integrate with SPEC-057 service account auth
- [ ] Add vertex config fields to ProviderKeyEntry
- [ ] Register executor
- [ ] Unit tests
- [ ] Integration tests

## Test Strategy

- **Unit tests:** URL construction, config parsing
- **Integration tests:** Mock Vertex AI endpoint
