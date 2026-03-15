# Technical Design: Provider Families & Auth Profiles

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-067       |
| Title     | Provider Families & Auth Profiles |
| Author    | AI Agent       |
| Status    | Completed      |
| Created   | 2026-03-15     |
| Updated   | 2026-03-15     |

## Overview

This change introduces logical provider families with nested authentication profiles. Each provider family owns model catalog and protocol behavior. Each auth profile owns credential material, health identity, region/weight hints, and presentation overrides. Runtime dispatch still executes against flattened `AuthRecord`s, but those records now represent `provider + auth-profile` rather than a single config row.

Reference: [PRD](prd.md)

## API Design

### New Endpoints

```
GET  /api/dashboard/auth-profiles
POST /api/dashboard/auth-profiles
PUT  /api/dashboard/auth-profiles/{provider}/{profile}
DELETE /api/dashboard/auth-profiles/{provider}/{profile}
POST /api/dashboard/auth-profiles/codex/oauth/start
POST /api/dashboard/auth-profiles/codex/oauth/complete
POST /api/dashboard/auth-profiles/{provider}/{profile}/refresh
```

### Request/Response Shape

```json
{
  "provider": "openai-codex",
  "profile_id": "personal",
  "redirect_uri": "http://localhost:1455/auth/callback"
}
```

```json
{
  "auth_url": "https://auth.openai.com/oauth/authorize?...",
  "state": "opaque-state"
}
```

## Backend Implementation

### Module Structure

```
crates/core/src/
├── auth_profile.rs
├── config.rs
└── provider.rs

crates/server/src/handler/dashboard/
├── auth_profiles.rs
└── providers.rs
```

### Key Types

```rust
pub enum AuthMode {
    ApiKey,
    BearerToken,
    OpenaiCodexOauth,
}

pub enum AuthHeaderKind {
    Auto,
    Bearer,
    XApiKey,
    XGoogApiKey,
}

pub struct AuthProfileEntry {
    pub id: String,
    pub mode: AuthMode,
    pub header: AuthHeaderKind,
    pub secret: Option<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_at: Option<String>,
    pub headers: HashMap<String, String>,
    pub disabled: bool,
    pub weight: u32,
    pub region: Option<String>,
    pub prefix: Option<String>,
    pub upstream_presentation: UpstreamPresentationConfig,
}
```

### Flow

1. Config loads provider families.
2. Each provider family expands into one or more runtime auth profiles.
3. `CredentialRouter` flattens those into `AuthRecord`s grouped by provider name.
4. Dispatch plans routes across providers, then credentials within a provider.
5. Before execution, auth runtime resolves the current request secret and refreshes Codex OAuth tokens when needed.
6. Runtime OAuth material is persisted into a sidecar auth runtime store (`*.auth-runtime.json`) instead of being rewritten into YAML config.
7. Executors apply the resolved auth header kind instead of assuming auth from base URL alone.

## Configuration Changes

```yaml
providers:
  - name: anthropic
    format: claude
    models:
      - id: claude-sonnet-4-5
    auth-profiles:
      - id: billing
        mode: api-key
        secret: env://ANTHROPIC_API_KEY
      - id: subscription-main
        mode: bearer-token
        secret: env://OPENCLAW_ANTHROPIC_SETUP_TOKEN

  - name: openai-codex
    format: openai
    wire-api: responses
    models:
      - id: gpt-5-codex
    auth-profiles:
      - id: personal
        mode: openai-codex-oauth
```

Runtime OAuth material is stored outside YAML in a sidecar file derived from the config path:

```text
config.yaml
config.auth-runtime.json
```

## Provider Compatibility

| Provider | Supported | Notes |
|----------|-----------|-------|
| OpenAI   | Yes       | API key and Codex OAuth profiles |
| Claude   | Yes       | API key and bearer-token profiles |
| Gemini   | Yes       | API key and bearer-token profiles |

## Alternative Approaches

| Approach | Pros | Cons | Verdict |
|----------|------|------|---------|
| Top-level `auth_profiles[]` separate from providers | Very explicit model | Larger migration blast radius | Rejected for this iteration |
| Reuse `credential_source` only | Small code diff | Cannot represent refreshable auth lifecycle | Rejected |
| Full OpenClaw state store clone | Maximum fidelity | Too much surface for one iteration | Deferred |

## Task Breakdown

- [x] Add auth profile core types and config parsing
- [x] Flatten provider families into runtime `AuthRecord`s
- [x] Add auth header kind and Codex OAuth token refresh runtime
- [x] Update executors and count-tokens path to use resolved auth headers
- [x] Add dashboard auth profile APIs
- [x] Add dashboard UI pages for auth profile management and OAuth callback handling
- [x] Add config, routing, dashboard, web, and request-path tests

## Test Strategy

- **Unit tests:** config normalization, auth profile expansion, auth runtime refresh, executor auth header selection
- **Integration tests:** dashboard auth profile APIs, provider create/update with nested auth profiles, route selection across multiple profiles
- **Web E2E:** Playwright coverage for login, auth profile CRUD, OAuth callback, provider flows, and system/routing pages
- **Manual verification:** local and remote-machine Codex OAuth start/complete flow and Anthropic bearer-token profile request

## Rollout Outcome

1. Landed core auth profile data model and runtime resolver.
2. Landed dashboard endpoints, auth runtime sidecar persistence, and provider writeback migration.
3. Added multi-profile routing, OAuth refresh, dashboard API, frontend unit, and Playwright coverage.
4. Shipped web dashboard auth profile management and OAuth callback UX in the same iteration.
