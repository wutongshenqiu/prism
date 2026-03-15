# PRD: Dashboard Correctness & Control Plane Quality Overhaul

| Field     | Value                                              |
|-----------|----------------------------------------------------|
| Spec ID   | SPEC-066                                           |
| Title     | Dashboard Correctness & Control Plane Quality Overhaul |
| Author    | Codex                                              |
| Status    | Active                                             |
| Created   | 2026-03-15                                         |
| Updated   | 2026-03-15                                         |

## Problem Statement

The dashboard and control-plane surface currently contains several truthfulness and maintainability failures:

- frontend and backend routing introspection contracts have drifted apart, leaving Replay partially broken
- dashboard config writes can silently clobber each other under concurrent edits
- system health mostly reflects config enablement, not runtime health
- realtime request logs ignore active filters and degrade when dashboard auth expires
- config workspace and multiple dashboard pages present data models that do not match backend reality
- frontend tests cover only a small part of the dashboard, so contract drift and semantic regressions are easy to ship

These are not legacy constraints to preserve. The goal is to make the dashboard correct, explicit, and maintainable, even if that requires breaking existing internal dashboard payloads or replacing stale UI assumptions.

## Goals

- Make dashboard data truthful: every control-plane page must reflect real backend state, not placeholders or stale assumptions.
- Collapse duplicated dashboard contracts so frontend and backend share one canonical model per workflow.
- Replace best-effort config edits with deterministic, serialized config transactions.
- Make realtime dashboard behavior robust under filters, reconnects, and token expiry.
- Raise dashboard test coverage around real workflows and contract fidelity.

## Non-Goals

- Preserving existing internal dashboard request or response shapes for compatibility.
- Incremental patching that keeps misleading UI semantics alive.
- Expanding public inference APIs beyond what is required to make the dashboard truthful.

## User Stories

- As an operator, I want Replay and Route Preview to explain routing decisions using the same fields the backend actually returns, so I can trust the tooling.
- As an operator, I want dashboard edits to fail loudly on conflicts instead of silently overwriting another change.
- As an operator, I want System, Protocols, Models, and Config pages to describe runtime truth, not configuration guesses.
- As an operator, I want live request logs to remain useful when I am filtering by provider, tenant, or status.
- As a maintainer, I want dashboard tests to fail when contracts drift or when semantic regressions are introduced.

## Success Metrics

- No dashboard page reads fields that are absent from its backend API.
- Concurrent dashboard config writes are serialized or rejected with an explicit conflict response; no silent last-writer-wins overwrite.
- System health and capability views are derived from runtime inventory and health state instead of `disabled` flags alone.
- Realtime logs preserve active filtering semantics and recover cleanly from dashboard token refresh.
- Dashboard test coverage includes Routing, Replay, Config, System, Protocols, Models, and Tenants flows.

## Constraints

- Functionality and code quality take priority over backward compatibility for internal dashboard APIs.
- Routing introspection must use the same planner and runtime state as production dispatch.
- Config writes must preserve secret references such as `env://` and `file://`.
- Dashboard changes must remain scoped to control-plane behavior and must not regress public inference endpoints.

## Open Questions

- [ ] Should `preview` remain a separate endpoint from `explain`, or should the dashboard converge on a single introspection endpoint with optional detail levels?
- [ ] Should frontend dashboard types remain handwritten, or should they be generated/validated against backend JSON fixtures?

## Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|--------------------|--------|-----------|
| Dashboard API compatibility | Preserve current mixed shapes vs break internal payloads | Break internal dashboard payloads as needed | Internal correctness is more valuable than preserving stale contracts |
| Config mutation model | Distributed read/modify/write helpers vs single transaction path | Single transaction path | Centralizes locking, conflict detection, and auditability |
| Health semantics | Config-derived status vs runtime-derived status | Runtime-derived status | Operators need truth, not configuration summaries |
| Test strategy | Keep mostly store/component tests vs add workflow and contract tests | Add workflow and contract tests | Current failures are caused by contract drift and semantic gaps |
