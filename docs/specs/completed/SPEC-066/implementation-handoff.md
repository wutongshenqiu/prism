# SPEC-066 Implementation Handoff

## Goal

Implement SPEC-066 as a dashboard/control-plane correctness overhaul.

Guiding rules:

- prioritize functionality and code quality over backward compatibility
- do not preserve stale internal dashboard payloads
- prefer deleting misleading UI behavior over patching around it
- keep frontend/backend contracts canonical and explicit

## GitHub Tracking

- Epic: `#257`
- `#258` Unify routing preview/explain contracts and rebuild Replay
- `#259` Serialize dashboard config writes and add explicit conflict handling
- `#260` Make System, Protocols, and Models pages reflect runtime truth
- `#261` Make realtime request logs filter-aware and auth-resilient
- `#262` Rework Config workspace around a truthful sanitized config model
- `#263` Clean up dashboard design tokens and semantic badges
- `#264` Expand dashboard contract, integration, and browser workflow tests

## Recommended Execution Order

### Phase 1: Foundation

1. `#258` Route introspection contract unification
2. `#259` Config transaction serialization

Reason:

- these are the highest-risk correctness items
- other dashboard pages should build on the new canonical contracts and mutation path

### Phase 2: Truthful Data Surfaces

3. `#260` Runtime-truthful System / Protocols / Models pages
4. `#262` Truthful Config workspace

Reason:

- both depend on deciding what backend truth models should look like
- both should consume stable backend payloads rather than frontend assumptions

### Phase 3: Realtime and UI Semantics

5. `#261` Filter-aware realtime logs and auth-resilient WebSocket flow
6. `#263` Design token and semantic badge cleanup

Reason:

- these are important, but should follow the core contract/data-model cleanup

### Phase 4: Test Closure

7. `#264` Dashboard contract, integration, and browser workflow tests

Reason:

- do this after payloads and semantics settle, otherwise tests will churn

## File Hotspots

### Backend

- `crates/server/src/handler/dashboard/control_plane.rs`
- `crates/server/src/handler/dashboard/routing.rs`
- `crates/server/src/handler/dashboard/providers.rs`
- `crates/server/src/handler/dashboard/config_ops.rs`
- `crates/server/src/handler/dashboard/system.rs`
- `crates/server/src/handler/dashboard/websocket.rs`
- `crates/server/tests/dashboard_tests.rs`

### Frontend

- `web/src/services/api.ts`
- `web/src/types/index.ts`
- `web/src/pages/Replay.tsx`
- `web/src/components/routing/RoutePreview.tsx`
- `web/src/pages/System.tsx`
- `web/src/pages/Protocols.tsx`
- `web/src/pages/ModelsCapabilities.tsx`
- `web/src/pages/Config.tsx`
- `web/src/pages/RequestLogs.tsx`
- `web/src/stores/logsStore.ts`
- `web/src/services/websocket.ts`
- `web/src/hooks/useWebSocket.ts`
- `web/src/index.css`
- `web/src/App.css`

## Definition Of Done

- Replay and Route Preview use one canonical request/response model
- dashboard config writes are serialized or reject stale mutations explicitly
- System / Protocols / Models / Config pages reflect backend truth
- live request logs do not violate active filters
- dashboard WebSocket flow handles auth expiry cleanly
- no undefined CSS variables remain in dashboard code
- backend and frontend tests cover the new contracts and workflows

## Verification Commands

```sh
cargo test -p prism-server --test dashboard_tests
npm test -- --run
```

If the implementation changes shared Rust behavior beyond dashboard handlers, also run:

```sh
cargo test --workspace
cargo clippy --workspace --tests -- -D warnings
```

## Claude Code Prompt

```text
Implement SPEC-066 in the Prism repo.

Context:
- Spec docs:
  - docs/specs/active/SPEC-066/prd.md
  - docs/specs/active/SPEC-066/technical-design.md
  - docs/specs/active/SPEC-066/implementation-handoff.md
- GitHub tracking:
  - epic #257
  - issues #258, #259, #260, #261, #262, #263, #264

Non-negotiable rules:
- prioritize functionality and code quality over backward compatibility
- do not preserve stale internal dashboard API shapes just to avoid churn
- if frontend and backend disagree, define one canonical model and migrate both sides
- prefer removing misleading UI behavior over keeping partial or fake semantics

Execution order:
1. #258 unify routing preview/explain contracts and rebuild Replay
2. #259 replace dashboard config mutations with one serialized transaction path and explicit conflict handling
3. #260 make System / Protocols / Models pages reflect runtime truth
4. #262 rework Config workspace around a truthful sanitized config model
5. #261 make realtime request logs filter-aware and auth-resilient
6. #263 clean up dashboard design tokens and semantic badges
7. #264 expand automated tests for dashboard contracts and workflows

Working requirements:
- inspect the current implementations before editing
- update both backend and frontend in the same change set when a contract changes
- delete obsolete frontend types and rendering paths after canonicalizing contracts
- add or update tests as each issue is completed
- do not leave TODO-based partial migrations

Minimum verification before finishing:
- cargo test -p prism-server --test dashboard_tests
- npm test -- --run

At the end:
- summarize what changed by issue number
- list any remaining risks or follow-ups only if they are real blockers
```
