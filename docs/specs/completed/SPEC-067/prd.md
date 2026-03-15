# PRD: Provider Families & Auth Profiles

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-067       |
| Title     | Provider Families & Auth Profiles |
| Author    | AI Agent       |
| Status    | Completed      |
| Created   | 2026-03-15     |
| Updated   | 2026-03-15     |

## Problem Statement

Prism's current unified `providers[]` config collapses logical provider identity and concrete credentials into the same record. That prevents a single provider family from hosting multiple authentication profiles, makes OAuth-style token lifecycle management awkward, and creates ambiguity when the same model namespace is offered by multiple upstreams. The current runtime already has a credential router, but configuration still forces one provider entry per credential.

## Goals

- Separate logical provider identity from authentication profiles while keeping the runtime dispatch model efficient.
- Support both static API keys and OpenClaw-style subscription/OAuth-backed auth material in the same gateway.
- Make provider/profile conflict resolution explicit and deterministic.
- Preserve the existing execution pipeline shape: translator -> presentation -> executor.
- Add management APIs and tests for auth profile lifecycle and request routing.

## Non-Goals

- Full browser-based OAuth orchestration for every provider on day one.
- Replacing the existing route planner or translator architecture.
- Changing the public `/v1/*` request protocols to force provider-qualified model IDs.

## User Stories

- As an operator, I want one `anthropic` provider with multiple auth profiles, such as a billing API key and multiple subscription tokens, so that routing and health are managed within one provider family.
- As an operator, I want one `openai-codex` provider with refreshable OAuth profiles so that Codex traffic can be routed separately from platform API key traffic.
- As an operator, I want to prefer or pin a specific auth profile for a request so that quota and identity are controlled explicitly.
- As a gateway maintainer, I want request routing to remain deterministic when multiple providers or profiles claim the same model.

## Success Metrics

- One provider entry can host multiple auth profiles at runtime.
- Static API key and refreshable Codex OAuth auth material can both be exercised by tests.
- Route planner and credential router select among providers and auth profiles deterministically.
- Dashboard APIs can create, inspect, and update auth profiles without editing raw YAML manually.
- Dashboard UI exposes auth profile CRUD and Codex OAuth flows end-to-end.

## Constraints

- Runtime execution remains based on resolved `AuthRecord` instances.
- Existing transport executors stay responsible for protocol invariants.
- Config reload and dashboard writeback must remain atomic.

## Resolved For This Iteration

- Anthropic subscription-style auth remains a static bearer-token profile in this iteration; provider-specific exchange flows are deferred.
- `/v1/models` remains flat and de-duplicated by exposed model ID in this iteration; provider-qualified IDs remain a future product decision.

## Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|--------------------|--------|-----------|
| Provider/auth modeling | Replace `providers[]` entirely vs nest auth profiles under providers | Nest auth profiles under logical providers | Minimizes churn in planner/router while fixing the data model |
| OpenClaw support | Re-implement full OpenClaw store vs support compatible auth material and Codex refresh flow | Support compatible auth material plus native Codex refresh | Covers the functional auth surface with lower implementation risk |
| Runtime token updates | Rewrite config on every refresh vs shared in-memory token state | Shared in-memory token state plus management write APIs | Keeps request path fast and avoids constant config rewrites |
