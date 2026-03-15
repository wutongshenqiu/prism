# Technical Design: Managed Auth Profiles & Claude Subscription Tokens

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-068       |
| Title     | Managed Auth Profiles & Claude Subscription Tokens |
| Author    | AI Agent       |
| Status    | Completed      |
| Created   | 2026-03-15     |
| Updated   | 2026-03-15     |

## Overview

Prism will keep the existing auth profile surface but evolve it into a provider-aware managed-auth system. `openai-codex-oauth` remains a refreshable managed auth mode, and a new `anthropic-claude-subscription` mode stores a Claude setup-token in the runtime sidecar instead of `config.yaml`.

## API Design

### New/Updated Endpoints

```text
POST /api/dashboard/auth-profiles/{provider}/{profile}/connect
POST /api/dashboard/auth-profiles/{provider}/{profile}/refresh
POST /api/dashboard/auth-profiles/codex/oauth/start
POST /api/dashboard/auth-profiles/codex/oauth/complete
```

### Connect Request

```json
{
  "secret": "sk-ant-oat01-..."
}
```

### Connect Response

```json
{
  "profile": {
    "provider": "anthropic-primary",
    "id": "subscription",
    "mode": "anthropic-claude-subscription",
    "connected": true
  }
}
```

## Backend Implementation

### Key Changes

- Extend `AuthMode` with `anthropic-claude-subscription`.
- Add mode helpers so the backend can treat Codex OAuth and Claude subscription credentials as managed auth profiles.
- Reuse the runtime sidecar store for managed credential material.
- Restrict Claude subscription profiles to `Format::Claude` providers targeting the official Anthropic host.
- Force Claude subscription auth to use `x-api-key`.
- Block protected upstream auth headers from request/profile header injection.

### Flow

1. Operator creates an auth profile with mode `anthropic-claude-subscription`.
2. Prism stores only profile metadata in config.
3. Operator connects the profile by posting the setup-token to the dashboard `connect` endpoint.
4. Prism validates token shape, validates the provider host, and writes the token into the runtime sidecar store.
5. Routing builds an `AuthRecord` whose effective secret comes from runtime state.
6. Claude executor sends the token via `x-api-key` to `https://api.anthropic.com`.

## Configuration Changes

```yaml
providers:
  - name: anthropic-primary
    format: claude
    base-url: https://api.anthropic.com
    auth-profiles:
      - id: subscription
        mode: anthropic-claude-subscription
```

## Provider Compatibility

| Provider | Supported | Notes |
|----------|-----------|-------|
| OpenAI   | Yes       | Existing Codex OAuth flow remains refreshable |
| Claude   | Yes       | New managed subscription token flow, official Anthropic host only |
| Gemini   | Yes       | Unchanged |

## Alternative Approaches

| Approach | Pros | Cons | Verdict |
|----------|------|------|---------|
| Reverse-engineered Claude browser OAuth | Fancy UX, no manual token paste | Fragile, undocumented, security risk | Rejected |
| Store Claude token in `config.yaml` | Simple implementation | Bad secret hygiene, poor dashboard UX | Rejected |
| Reuse generic bearer-token mode | Minimal code change | No provider guardrails, poor runtime secret isolation | Rejected |

## Task Breakdown

- [x] Extend core auth profile model with Claude managed mode and provider validation helpers
- [x] Tighten upstream protected header handling
- [x] Update auth runtime and dashboard auth profile handlers for Claude managed connect flow
- [x] Update provider CRUD runtime-state stripping/seeding logic
- [x] Update dashboard frontend for Claude managed profile UX
- [x] Add backend tests for Claude managed auth and header protection

## Test Strategy

- **Unit tests:** auth mode header resolution, provider validation, protected header filtering
- **Integration tests:** dashboard connect flow, sidecar persistence, Codex refresh regression coverage
- **Manual verification:** create Claude managed profile, connect a setup-token, ensure secret is absent from config and present in runtime store

## Rollout Plan

1. Land the managed-auth backend changes and tests.
2. Land dashboard support for Claude subscription connect flow.
3. Document operator guidance and security constraints.
