# PRD: Routing Config & Core Types

| Field     | Value                              |
|-----------|------------------------------------|
| Spec ID   | SPEC-048                           |
| Title     | Routing Config & Core Types        |
| Author    | Claude                             |
| Status    | Draft                              |
| Created   | 2026-03-14                         |
| Updated   | 2026-03-14                         |

## Problem Statement

Current `RoutingConfig` mixes model rewrite, model fallback, provider discovery, credential selection, and retry into a single flat structure. `routing.strategy` sounds global but only affects credential choice inside a provider. Weight is only used by weighted round-robin but the UI suggests otherwise. `default_region` exists but is unused at runtime.

The new config model must separate routing intent (profiles), request matching (rules), and model naming (model-resolution) into independent, composable concerns.

## Goals

- Replace `RoutingConfig` and `RoutingStrategy` with a profile-based, layered config model
- Separate provider-level policy from credential-level policy
- Define 4 presets (Balanced, Stable, Lowest Latency, Lowest Cost)
- Define route rule matching with explicit precedence
- Define model resolution (aliases, rewrites, fallbacks, provider-pins) as independent config
- Define all runtime types for route planning and tracing

## Non-Goals

- Runtime behavior changes (this spec is types only)
- Dashboard UI changes
- Health state implementation

## User Stories

- As an operator, I want to configure routing as a named profile so that I can reuse policies across rules.
- As an operator, I want to match requests by model/tenant/endpoint/headers and bind them to different profiles.
- As an operator, I want model aliasing, rewriting, fallback chains, and provider pinning as first-class config.

## Success Metrics

- All config types compile and serialize/deserialize correctly
- 4 preset profiles produce valid default configs
- Config YAML round-trips without data loss

## Constraints

- Types live in `crates/core/`
- No runtime side effects in this spec

## Open Questions

- None

## Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|--------------------|--------|-----------|
| Config location | New `routing/` submodule vs inline in config.rs | New `routing/` submodule | Separation of concerns, file size management |
| Preset representation | Hardcoded vs config-defined | Config-defined with factory defaults | Flexibility, users can customize presets |
