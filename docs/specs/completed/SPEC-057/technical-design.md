# Technical Design: OAuth & Auth-File Provider Authentication

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-057       |
| Title     | OAuth & Auth-File Provider Authentication |
| Author    | AI Agent       |
| Status    | Draft          |
| Created   | 2026-03-14     |
| Updated   | 2026-03-14     |

## Overview

Extends Prism's credential system to support non-static credential sources: auth files, OAuth2 client credentials, and GCP service accounts. A background CredentialRefresher service manages token lifecycle.

## Backend Implementation

### Key Types

```rust
// crates/core/src/credential_source.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum CredentialSource {
    Static { api_key: String },
    AuthFile { path: PathBuf, format: AuthFileFormat },
    OAuth2 { client_id: String, client_secret: String, token_url: String, scopes: Vec<String> },
    ServiceAccount { path: PathBuf },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthFileFormat {
    ClaudeCli,
    GcloudAuth,
    Custom { key_path: String },
}
```

### Configuration

```yaml
claude-api-key:
  - auth-file: ~/.claude/credentials.json
    auth-file-format: claude-cli
  - oauth:
      client-id: "..."
      client-secret: "env://CLIENT_SECRET"
      token-url: "https://oauth2.googleapis.com/token"
```

## Provider Compatibility

| Provider | Supported | Notes |
|----------|-----------|-------|
| Claude   | Yes       | auth-file (Claude CLI credentials) |
| OpenAI   | Yes       | Static keys (existing), future auth-file |
| Gemini   | Yes       | Service account, OAuth2 |

## Task Breakdown

- [ ] Create CredentialSource enum and types
- [ ] Implement auth-file reader (Claude CLI format)
- [ ] Implement OAuth2 client-credentials flow
- [ ] Implement GCP service account JWT flow
- [ ] Create CredentialRefresher background service
- [ ] Extend ProviderKeyEntry config parsing
- [ ] Integrate with CredentialRouter
- [ ] Add dashboard UI for new credential types
- [ ] Unit tests for each credential source
- [ ] Integration tests with mock OAuth server

## Test Strategy

- **Unit tests:** Auth file parsing, JWT generation, OAuth token exchange
- **Integration tests:** Mock OAuth server, file watch triggers
- **Manual verification:** Test with real Claude CLI credentials

## Rollout Plan

1. Add CredentialSource types
2. Implement auth-file reader
3. Add OAuth2 flow
4. Add service account support
5. Background refresher
6. Dashboard UI
