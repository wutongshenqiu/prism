# PRD: Execution Controller & Dispatch Cutover

| Field     | Value                                               |
|-----------|-----------------------------------------------------|
| Spec ID   | SPEC-051                                            |
| Title     | Execution Controller & Dispatch Cutover              |
| Author    | Claude                                              |
| Status    | Draft                                               |
| Created   | 2026-03-14                                          |
| Updated   | 2026-03-14                                          |

## Problem Statement

Current `dispatch.rs` embeds routing logic inline: it loops through providers, calls `router.pick()`, handles retries with exponential backoff, and mixes route selection with request execution. Retry is not stage-aware (credential hop vs provider hop vs model hop share the same counter). There is no per-try timeout, no retry budget enforcement, and no structured route trace output.

## Goals

- Replace dispatch inline routing with `ExecutionController` that consumes a `RoutePlan`
- Implement stage-aware failover: credential retry -> provider failover -> model fallback
- Each stage has independent attempt limits
- Add per-try timeout
- Enforce retry budget
- Produce `RouteTrace` events for every attempt
- Wire route trace into request logs and debug headers

## Non-Goals

- Translation/cloaking/payload rules logic changes
- Dashboard UI (SPEC-052)

## User Stories

- As an operator, I want credential retries, provider failover, and model fallback to have separate attempt limits.
- As an operator, I want per-try timeouts so a slow upstream doesn't consume my entire timeout budget.
- As an operator, I want retry budgets so retries don't amplify upstream load.
- As an operator, I want to see the full route trace in request logs when debugging.

## Success Metrics

- Dispatch no longer contains route selection logic
- Each failover stage has its own counter visible in traces
- Route trace appears in request logs
- Debug headers contain route summary from trace

## Constraints

- Must not change translation, cloaking, or payload rules logic
- Must preserve streaming and keepalive behavior
- `ExecutionController` consumes `RoutePlan` from planner, does not build routes

## Open Questions

- None

## Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|--------------------|--------|-----------|
| Execution model | Sequential attempts vs pre-built plan | Pre-built plan from planner | Separation of planning and execution |
| Retry scope | Global counter vs per-stage | Per-stage counters | Finer control, matches TD requirement |
