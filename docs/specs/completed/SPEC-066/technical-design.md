# Technical Design: Dashboard Correctness & Control Plane Quality Overhaul

| Field     | Value                                              |
|-----------|----------------------------------------------------|
| Spec ID   | SPEC-066                                           |
| Title     | Dashboard Correctness & Control Plane Quality Overhaul |
| Author    | Codex                                              |
| Status    | Active                                             |
| Created   | 2026-03-15                                         |
| Updated   | 2026-03-15                                         |

## Overview

This spec hardens the dashboard as a real control plane instead of a best-effort UI wrapper. The work spans six areas:

1. Route introspection contract unification
2. Serialized config transactions
3. Runtime-truthful health and capability reporting
4. Filter-aware, auth-resilient realtime logs
5. Truthful config workspace and dashboard UI semantics
6. Test coverage expansion around contracts and workflows

The implementation is allowed to replace stale internal payloads and UI assumptions outright.

## API Design

The exact payload shapes will be finalized during implementation, but the following constraints are mandatory:

### Routing Introspection

- `POST /api/dashboard/routing/preview`
- `POST /api/dashboard/routing/explain`

Requirements:

- both endpoints must consume a shared canonical request type
- both endpoints must return shapes explicitly modeled in `web/src/types` or a generated equivalent
- Replay and Route Preview must render only fields guaranteed by the backend response
- stale response concepts such as frontend-only `score` or `model_resolution` fields must be either implemented server-side or removed from the UI

### Config Mutation

- `POST /api/dashboard/providers`
- `PATCH /api/dashboard/providers/{id}`
- `DELETE /api/dashboard/providers/{id}`
- `PATCH /api/dashboard/routing`
- `PUT /api/dashboard/config/apply`

Requirements:

- all dashboard config mutations must flow through one transaction helper
- transaction helper must provide serialization and conflict handling
- conflict responses must be explicit and machine-readable

### Runtime Truth Endpoints

- `GET /api/dashboard/system/health`
- `GET /api/dashboard/providers/capabilities`
- `GET /api/dashboard/protocols/matrix`
- `GET /api/dashboard/config/current`

Requirements:

- responses must expose actual runtime or sanitized config truth
- pages must stop inferring semantics from partial summaries

## Backend Implementation

### Module Structure

```
crates/server/src/handler/dashboard/
├── config_ops.rs
├── control_plane.rs
├── providers.rs
├── routing.rs
├── system.rs
└── websocket.rs

crates/server/src/dashboard/
└── config_tx.rs          # new shared config transaction helper
```

### Key Types

```rust
struct DashboardConfigVersion {
    path: String,
    sha256: String,
}

struct ConfigTransactionResult {
    version: DashboardConfigVersion,
}

struct RouteIntrospectionRequest {
    model: String,
    endpoint: RouteEndpoint,
    source_format: Format,
    tenant_id: Option<String>,
    api_key_id: Option<String>,
    region: Option<String>,
    stream: bool,
    feature_flags: RouteExplainFeatures,
}
```

### Flow

#### 1. Route introspection

1. Frontend submits a canonical introspection request.
2. Backend converts it to `RouteRequestFeatures` using one code path.
3. Planner runs against live routing config, provider catalog, and health manager snapshot.
4. Backend returns a response shape that both Route Preview and Replay can render directly.

#### 2. Config transactions

1. Mutation handlers load the raw config plus a version hash.
2. A single helper applies the mutation, validates the result, and writes through a uniquely named temp file.
3. The helper either serializes updates under a lock or rejects stale versions with an explicit conflict.
4. Runtime state is refreshed once, through a shared reload path.

#### 3. Runtime truth reporting

1. System health aggregates provider runtime health, not only config enablement.
2. Capability pages consume capability and protocol data emitted by backend truth sources.
3. Config workspace reads a sanitized config shape that matches the UI sections exactly, or the UI is simplified to match the available model.

#### 4. Realtime logs

1. WebSocket messages are filtered client-side against the active filter set before insertion, or the client falls back to refetch when realtime semantics would be ambiguous.
2. Dashboard token refresh is integrated with WS reconnect.
3. The UI surfaces disconnected / reconnecting states explicitly.

## Configuration Changes

No new user-facing YAML fields are required. Internal dashboard APIs may gain:

- version/hash fields for optimistic concurrency
- normalized introspection request/response fields
- explicit WS connection status or auth-failure signals

## Provider Compatibility

| Provider | Supported | Notes |
|----------|-----------|-------|
| OpenAI   | Yes | Public inference behavior unchanged; dashboard reporting becomes more accurate |
| Claude   | Yes | Same as above |
| Gemini   | Yes | Same as above |

## Alternative Approaches

| Approach | Pros | Cons | Verdict |
|----------|------|------|---------|
| Patch each page independently | Low initial effort | Preserves drift and duplicate models | Rejected |
| Preserve existing dashboard payloads and shim frontend | Lower short-term churn | Bakes stale contracts into new code | Rejected |
| Full dashboard rewrite before hardening | Maximum cleanup | Too much scope at once | Rejected |
| Canonicalize contracts and transaction paths incrementally | Fixes core defects while keeping scope bounded | Requires coordinated frontend/backend changes | Chosen |

## Task Breakdown

- [ ] Unify route introspection contracts and rebuild Replay on the canonical API.
- [ ] Replace dashboard config mutations with a serialized transaction helper and explicit conflict handling.
- [ ] Make System, Protocols, and Models pages reflect runtime truth instead of config-derived assumptions.
- [ ] Make realtime request logs filter-aware and robust across dashboard token expiry.
- [ ] Rework Config workspace around a truthful sanitized config model.
- [ ] Clean up dashboard design tokens and semantic badges so visuals reflect real state.
- [ ] Add backend, frontend, and browser-level tests for dashboard contracts and workflows.

## Test Strategy

- **Unit tests:** transaction helper conflict handling, route introspection request/response mapping, frontend stores and components for realtime behavior.
- **Integration tests:** dashboard handler tests for routing introspection, config conflicts, runtime health semantics, config workspace shapes.
- **Browser tests:** login, routing preview, replay, config workspace, system health, protocols/models/tenants pages using deterministic seeded fixtures instead of project-specific provider names or models.

## Rollout Plan

1. Merge the canonical route introspection and config transaction changes first.
2. Rebuild dependent dashboard pages to consume the new truth sources.
3. Land test fixture and E2E updates before considering the work complete.
