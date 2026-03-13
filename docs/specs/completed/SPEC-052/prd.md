# PRD: Preview API & Dashboard UX

| Field     | Value                                    |
|-----------|------------------------------------------|
| Spec ID   | SPEC-052                                 |
| Title     | Preview API & Dashboard UX               |
| Author    | Claude                                   |
| Status    | Draft                                    |
| Created   | 2026-03-14                               |
| Updated   | 2026-03-14                               |

## Problem Statement

There is no first-class "why was this route chosen?" API. The dashboard UI suggests stronger semantics than the code provides. Current routing page exposes a single strategy dropdown that does not reflect the actual routing behavior. Operators cannot preview or debug routing decisions without reading code or analyzing logs after the fact.

## Goals

- Add preview and explain API endpoints using the same planner as production
- Dashboard routing page with preset cards, rule table, advanced editor, and route preview
- Show human-intent labels before algorithm names
- Display effective policy, not raw config fragments
- Validate impossible combinations in UI before save
- Remove legacy routing UI and obsolete config fields

## Non-Goals

- Semantic routing
- Custom strategy plugins

## User Stories

- As an operator, I want to select a routing preset (Balanced/Stable/Lowest Latency/Lowest Cost) and understand what it does in plain language.
- As an operator, I want to define rules that match requests by model/tenant/endpoint and bind them to profiles.
- As an operator, I want to preview a route by entering model/endpoint/tenant and seeing which credential would be selected and why.
- As an operator, I want to see why specific credentials were rejected (region mismatch, circuit breaker open, etc.).

## Success Metrics

- Preview API returns same result as production planner for identical inputs
- Operators can understand routing decisions without reading code
- Dashboard validates config before save

## Constraints

- Preview uses exact same planner as production (no separate preview logic)
- Route preview must work without executing upstream calls

## Open Questions

- None

## Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|--------------------|--------|-----------|
| Preview fidelity | Simplified preview vs same planner | Same planner | Eliminates preview/production divergence |
| UI approach | Single page vs multi-tab | Single page with sections | Progressive disclosure |
