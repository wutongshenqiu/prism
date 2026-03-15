# Technical Design: Codex Device Flow, Local Import, and Responses WebSocket

| Field     | Value |
|-----------|-------|
| Spec ID   | SPEC-070 |
| Title     | Codex Device Flow, Local Import, and Responses WebSocket |
| Author    | AI Agent |
| Status    | Completed |
| Created   | 2026-03-15 |
| Updated   | 2026-03-15 |

## Overview

This spec extends SPEC-069 with three production-grade Codex capabilities:

1. Managed auth can be connected by browser OAuth, device flow, or server-local CLI auth import.
2. Dashboard control-plane auth is reduced to bearer header or HttpOnly cookie only; query-token fallback is removed.
3. Prism exposes websocket Responses ingress and bridges it onto the existing dispatch plus SSE execution stack.

## API Design

### Dashboard

```text
POST /api/dashboard/auth-profiles/codex/device/start
POST /api/dashboard/auth-profiles/codex/device/poll
POST /api/dashboard/auth-profiles/{provider}/{profile}/import-local
```

### Public API

```text
GET /v1/responses/ws
GET /api/provider/{provider}/v1/responses/ws
```

## Backend Implementation

### Auth Runtime

- `AuthRuntimeManager` now owns Codex device endpoints and an optional runtime-specific auth-file path.
- Device flow is implemented as:
  1. request user code
  2. poll for authorization code + PKCE verifier
  3. exchange into managed runtime tokens
- Local import parses the Codex CLI auth bundle and extracts `access_token`, `refresh_token`, `id_token`, expiry, account ID, and email.

### Dashboard

- `auth_profiles.rs` adds `device/start`, `device/poll`, and `import-local`.
- `AppState` tracks short-lived device sessions separately from browser OAuth PKCE sessions.
- `dashboard_auth` no longer accepts `?token=` query parameters.

### Responses WebSocket

- `handler/responses_ws.rs` owns websocket request normalization, credential pinning, SSE-to-websocket forwarding, and transcript state.
- The websocket path reuses `dispatch()` with `responses_passthrough=true` and `stream=true`.
- The first successful turn captures `x-prism-route-credential`, pins that credential, and infers the upstream family.
- Codex preserves `previous_response_id` for follow-up websocket turns; other upstreams fall back to merged transcript input.

## Security

- Dashboard query-token auth is removed.
- Managed Codex tokens stay in the auth runtime sidecar only.
- Local auth import is restricted to the server-local Codex auth bundle path rather than arbitrary dashboard-supplied file reads.

## Test Strategy

- Rust unit tests: websocket request normalization
- Rust integration tests: device flow, local import, dashboard auth hardening
- Web tests: TypeScript + Vitest
- Playwright: dashboard Codex OAuth flow after connect-modal UX change
- Manual/live: real Codex local-import and `/v1/responses/ws` verification
