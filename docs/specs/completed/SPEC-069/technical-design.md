# Technical Design: Codex Native Upstream & Managed OAuth

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-069       |
| Title     | Codex Native Upstream & Managed OAuth |
| Author    | AI Agent       |
| Status    | Completed      |
| Created   | 2026-03-15     |
| Updated   | 2026-03-15     |

## Overview

Codex is split out as its own upstream family while still accepting OpenAI wire-format ingress. The runtime now distinguishes `format` from `upstream`, routes Codex credentials to a dedicated executor, and enforces Codex-specific auth and host policy. This spec supersedes the Codex-specific assumptions embedded in SPEC-067 and SPEC-068.

Reference: [PRD](prd.md)

## API Design

### Endpoints

```text
POST /api/dashboard/providers
PATCH /api/dashboard/providers/{id}
POST /api/dashboard/providers/fetch-models
POST /api/dashboard/providers/{id}/health
POST /api/dashboard/auth-profiles/codex/oauth/start
POST /api/dashboard/auth-profiles/codex/oauth/complete
POST /api/dashboard/auth-profiles/{provider}/{profile}/refresh
```

### Request / Response Shape

```json
{
  "name": "codex-gateway",
  "format": "openai",
  "upstream": "codex",
  "base_url": "https://chatgpt.com/backend-api/codex",
  "wire_api": "responses"
}
```

```json
{
  "name": "codex-gateway",
  "format": "openai",
  "upstream": "codex",
  "wire_api": "responses",
  "auth_profiles": [
    {
      "id": "personal",
      "mode": "codex-oauth",
      "connected": true
    }
  ]
}
```

## Backend Implementation

### Module Structure

```text
crates/core/src/
├── auth_profile.rs
├── config.rs
└── provider.rs

crates/provider/src/
├── codex.rs
├── common.rs
└── lib.rs

crates/server/src/
├── dispatch/executor.rs
└── handler/dashboard/
    ├── auth_profiles.rs
    └── providers.rs
```

### Key Types

```rust
pub enum UpstreamKind {
    OpenAI,
    Codex,
    Claude,
    Gemini,
}

pub enum AuthMode {
    ApiKey,
    BearerToken,
    CodexOAuth,
    AnthropicClaudeSubscription,
}
```

### Flow

1. Dashboard/provider config writes `format` plus optional `upstream`.
2. Config validation derives `upstream` from `format` when omitted and rejects invalid format/upstream combinations.
3. `CredentialRouter` expands provider entries into `AuthRecord`s carrying both `provider` and `upstream`.
4. Dispatch chooses an executor by `upstream`, not by ingress wire format alone.
5. Codex requests are executed by `CodexExecutor` against `https://chatgpt.com/backend-api/codex`.
6. Managed Codex OAuth tokens are stored and refreshed only in the auth runtime sidecar.
7. Dashboard health/model-discovery behavior becomes upstream-aware: Codex skips generic model discovery and uses a Codex-specific health probe.

## Configuration Changes

```yaml
providers:
  - name: codex-gateway
    format: openai
    upstream: codex
    base-url: https://chatgpt.com/backend-api/codex
    wire-api: responses
    models:
      - id: gpt-5
    auth-profiles:
      - id: personal
        mode: codex-oauth
```

## Provider Compatibility

| Provider | Supported | Notes |
|----------|-----------|-------|
| OpenAI API | Yes | Continues to use `upstream: openai` and the OpenAI-compatible executor |
| Codex | Yes | Uses `upstream: codex`, dedicated executor, runtime-only OAuth tokens |
| Claude | Yes | Unchanged aside from shared managed-auth validation framework |
| Gemini | Yes | Unchanged |

## Alternative Approaches

| Approach | Pros | Cons | Verdict |
|----------|------|------|---------|
| Keep Codex as generic OpenAI provider | Minimal diff | Wrong scopes, wrong base URL, weak security boundary | Rejected |
| Add a boolean `codex: true` flag | Smaller config diff | Creates overlapping semantics with `format` and executor lookup | Rejected |
| Reuse public OpenAI `/v1/*` probes for Codex | Simple implementation | Provably fails with real Codex credentials | Rejected |

## Task Breakdown

- [x] Introduce `UpstreamKind` and carry it through config, routing, and dashboard APIs.
- [x] Rename the managed Codex auth mode to `codex-oauth`.
- [x] Add a dedicated Codex executor and executor selection by upstream family.
- [x] Enforce Codex-specific validation and protected headers.
- [x] Update dashboard UI, web tests, and Playwright fixtures for the new contract.
- [x] Run the full dashboard verification bundle and finalize the spec status.

## Test Strategy

- **Unit tests:** Codex payload normalization, header protection, auth-profile validation.
- **Integration tests:** dashboard provider CRUD, Codex OAuth start/complete/refresh, runtime sidecar persistence.
- **Web tests:** TypeScript, Vitest, and Playwright dashboard coverage for provider/auth-profile flows.
- **Manual verification:** real deployment test against a live Prism instance after merge.

## Rollout Plan

1. Land the clean Codex upstream model with no legacy aliases.
2. Verify cargo, web, build, and Playwright suites.
3. Mark the spec completed once the verification bundle passes.
