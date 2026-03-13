# PRD: Route Planner & Match Engine

| Field     | Value                                    |
|-----------|------------------------------------------|
| Spec ID   | SPEC-049                                 |
| Title     | Route Planner & Match Engine             |
| Author    | Claude                                   |
| Status    | Draft                                    |
| Created   | 2026-03-14                               |
| Updated   | 2026-03-14                               |

## Problem Statement

Current routing decisions are embedded inside `dispatch.rs` and `CredentialRouter.pick()`. There is no way to preview a routing decision without executing an upstream call. The route planner must be a pure function that takes immutable snapshots and produces a deterministic `RoutePlan`.

## Goals

- Implement a pure, side-effect-free route planner
- Implement request-to-profile matching with explicit specificity-based precedence
- Implement model resolution (alias, rewrite, fallback chain, provider pin)
- Implement route explanation as structured data
- All planner logic must be deterministic and testable with snapshot-style tests

## Non-Goals

- Runtime health state (SPEC-050)
- Selection strategy algorithms (SPEC-050)
- Execution / failover (SPEC-051)
- Dashboard UI (SPEC-052)

## User Stories

- As an operator, I want to preview which route a request would take before it executes.
- As an operator, I want to understand why a specific credential was rejected with a structured reason.
- As an operator, I want model aliasing and rewriting to happen transparently before provider selection.

## Success Metrics

- Planner produces identical output for identical inputs (determinism)
- All routing decisions include structured reject reasons
- 100% unit test coverage for match engine specificity rules

## Constraints

- Planner must not hold mutable state
- Planner must not perform I/O
- Planner input is snapshots only (config, inventory, health)

## Open Questions

- None

## Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|--------------------|--------|-----------|
| Planner purity | Planner owns state vs planner is pure | Pure function | Testability, separation of concerns |
| Specificity | First-match vs most-specific | Most-specific wins | More predictable for operators |
