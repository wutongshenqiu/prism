# PRD: Health State & Selection Strategies

| Field     | Value                                         |
|-----------|-----------------------------------------------|
| Spec ID   | SPEC-050                                      |
| Title     | Health State & Selection Strategies            |
| Author    | Claude                                        |
| Status    | Draft                                         |
| Created   | 2026-03-14                                    |
| Updated   | 2026-03-14                                    |

## Problem Statement

Current `CredentialRouter` mixes credential inventory, health tracking (circuit breaker + EWMA latency), and selection strategy into one struct. Health state is limited to circuit breaker and EWMA latency. There is no outlier detection, no inflight tracking, no retry budget, no 429/5xx rate tracking, and no temporary ejection.

Selection strategies are limited to 4 options (RoundRobin, FillFirst, LatencyAware, GeoAware), all operating at credential level only. There is no provider-level selection policy.

## Goals

- Replace `CredentialRouter` with separated `HealthManager` + `ProviderCatalog` + selection strategies
- Implement comprehensive health tracking: circuit breaker, outlier detection, ejection, inflight, EWMA latency/cost, 429/5xx rates, cooldown
- Implement 5 provider selection strategies
- Implement 6 credential selection strategies with priority tiering
- Health state is read-only for selectors (write happens after attempt results)

## Non-Goals

- Dispatch integration (SPEC-051)
- Dashboard UI (SPEC-052)

## User Stories

- As an operator, I want unhealthy credentials to be temporarily ejected rather than waiting for circuit breaker to fully open.
- As an operator, I want provider selection to be independent from credential selection.
- As an operator, I want priority tiers so that backup credentials are only used when primary tier is exhausted.
- As an operator, I want inflight-aware routing so no single credential gets overwhelmed.

## Success Metrics

- HealthManager tracks all 8 health signals per credential
- Each selection strategy produces correct ordering in unit tests
- Priority tiering correctly groups and orders within tiers
- Retry budget correctly limits retry rate

## Constraints

- `HealthManager` does not choose routes; it only exposes state
- Selection strategies must be deterministic given the same health snapshot
- Priority tier logic applies before any credential strategy

## Open Questions

- None

## Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|--------------------|--------|-----------|
| Health ownership | Per-credential vs centralized | Centralized HealthManager | Single source of truth, easier snapshotting |
| Selector interface | Concrete fns vs trait | Trait-based | Testable, swappable |
