# PRD: Quota-Aware Credential Switching

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-061       |
| Title     | Quota-Aware Credential Switching |
| Author    | AI Agent       |
| Status    | Draft          |
| Created   | 2026-03-14     |
| Updated   | 2026-03-14     |

## Problem Statement

When upstream returns 429/quota-exceeded, the circuit breaker records failure but doesn't immediately exclude that credential. The next request may select the same exhausted credential, causing unnecessary failures.

## Goals

- On 429/quota-exceeded response, set temporary cooldown for the credential
- Cooldown duration from `Retry-After` header (existing `parse_retry_after`), default 60s
- `CredentialRouter` new `QuotaCooldown` tracking
- Skip cooled-down credentials during selection
- Config: `quota-cooldown-default-secs: 60`

## Non-Goals

- Predictive quota tracking
- Cross-credential quota pooling

## User Stories

- As an operator, I want exhausted credentials to be temporarily skipped to reduce failed requests.

## Success Metrics

- Zero unnecessary 429 retries after quota exhaustion
- Credentials automatically resume after cooldown

## Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|--------------------|--------|-----------|
| Cooldown storage | Separate map, extend circuit breaker, credential metadata | Separate DashMap | Clean separation of concerns |
| Default cooldown | 30s, 60s, 120s | 60s | Conservative but responsive |
