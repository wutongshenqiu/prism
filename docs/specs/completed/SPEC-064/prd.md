# PRD: Upstream Presentation Layer

| Field     | Value                              |
|-----------|------------------------------------|
| Spec ID   | SPEC-064                           |
| Title     | Upstream Presentation Layer        |
| Author    | AI Agent                           |
| Status    | Draft                              |
| Created   | 2026-03-14                         |
| Updated   | 2026-03-14                         |
| Parent    | GitHub Issue #203                  |
| Issues    | #206, #207, #208, #209             |

## Problem Statement

Prism currently has three uncoordinated mechanisms for shaping what upstream providers see:

1. **`headers`** (per-provider) -- raw key-value pairs injected into every upstream request for a credential. No semantic meaning, no validation, no provenance tracking.
2. **`cloak`** (per-provider, Claude-only) -- body-level mutations (system prompt injection, synthetic `user_id`, sensitive word obfuscation) applied during dispatch. Tightly coupled to Claude format.
3. **`claude-header-defaults`** (global) -- a global `HashMap<String, String>` injected into request headers only when cloaking is active for Claude targets. This is a format-specific global that bleeds across providers.

These three live at different scopes (global vs per-provider), target different layers (headers vs body), and are wired into different code paths (config globals, `AuthRecord`, `executor.rs` inline logic). The result:

- **No unified concept of "client identity"**: To present as Claude Code to Anthropic, an operator must configure `cloak`, `claude-header-defaults`, and possibly `headers` -- three separate knobs in two config scopes.
- **Body mutation and header synthesis are separate abstractions**: They should be one. A "client profile" that sets a `user-agent` header but also injects a `metadata.user_id` is a single identity concern, not two.
- **Claude-only design**: `cloak` and `claude-header-defaults` are hard-wired to `Format::Claude`. There is no equivalent for presenting as Gemini CLI or Codex CLI to their respective providers.
- **No inspection or preview**: There is no way to see what Prism would actually send upstream before a request is made. Debug mode shows routing info but not the effective outgoing presentation state.
- **Dashboard is header-first**: The provider edit form surfaces raw headers as a primary editing concern. There is no profile selector, no semantic grouping, no preview.

## Goals

1. **Profile-first model**: Operators select a named client profile (e.g., `claude-code`, `gemini-cli`, `codex-cli`) rather than manually assembling headers and body mutations. Raw header overrides remain available as an advanced escape hatch.
2. **One abstraction owns identity**: A single `upstream-presentation` config per provider owns both header synthesis and identity-driven body mutations. No more split between `cloak`, `headers`, and `claude-header-defaults`.
3. **Provider-agnostic extensibility**: The presentation engine works across all three formats (OpenAI, Claude, Gemini). Built-in profiles can declare format-specific behavior, but the engine itself is format-aware, not format-bound.
4. **Decoupled from executors**: Provider executors remain responsible only for auth headers, protocol invariants (e.g., `anthropic-version`, `content-type`), and transport. They do not know about profiles or identity semantics.
5. **Inspectable and testable**: The presentation engine is pure (no Axum, no reqwest, no side effects). Its output is deterministic and traceable. A preview API lets operators inspect effective outgoing state.
6. **Safe migration**: Existing `headers`, `cloak`, and `claude-header-defaults` configs continue to work via transparent normalization during config load. No configs break on upgrade.
7. **Protected headers**: Auth and protocol-invariant headers cannot be accidentally overridden by profiles or custom headers. The executor is the final authority for these headers.

## Non-Goals

- **Generic inbound header passthrough**: Prism does not forward arbitrary client headers upstream. Only headers explicitly produced by the presentation engine (profile + custom) are sent.
- **WebSocket presentation**: v1 covers HTTP request/response only. WebSocket identity presentation is deferred.
- **New crate**: v1 adds a `presentation` module under `prism_core`. Extraction to a standalone crate is deferred until a second independent consumer justifies it.
- **Runtime profile switching**: Profiles are static per-provider config. Dynamic per-request profile selection is not in scope.
- **Plugin/trait-based profile extensibility**: v1 uses a closed enum of built-in profiles. A trait-based plugin system for custom profiles is deferred.

## User Stories

1. **As an operator**, I want to select `upstream-presentation.profile: claude-code` on my Anthropic provider and have Prism automatically send the right identity headers and body mutations, without manually configuring `cloak`, `headers`, and `claude-header-defaults` separately.
2. **As an operator**, I want to present Prism as a Gemini CLI client to Google's API by setting `upstream-presentation.profile: gemini-cli`, getting appropriate `user-agent` and `x-goog-api-client` headers automatically.
3. **As an operator**, I want to preview the effective upstream headers and body mutations for a provider before deploying a config change, so I can verify Prism will present correctly.
4. **As an operator**, I want raw custom headers available as an escape hatch for edge cases that built-in profiles don't cover, without losing the semantic benefits of profile-first configuration.
5. **As an operator**, I want to be confident that my presentation config cannot accidentally override auth headers or protocol invariants, even if I misconfigure custom headers.
6. **As an operator**, I want my existing `cloak` and `headers` config to keep working after upgrading, with automatic transparent migration to the new `upstream-presentation` model.
7. **As a dashboard user**, I want the provider edit form to show a profile selector as the primary control, with profile-specific options below it, and raw headers collapsed in an advanced section.

## Success Metrics

- All current `cloak` + `claude-header-defaults` + `headers` behavior is reproducible via `upstream-presentation` config.
- Presentation engine is unit-testable without spinning up server or providers (zero transport dependencies).
- Built-in profiles produce deterministic, traceable output for all three formats.
- Protected header violations are logged in trace output and silently dropped (no runtime errors).
- Preview API returns the exact headers and body mutations that would be applied.
- No existing config files break on upgrade (backward compatibility).
- Dashboard profile editing is demonstrably simpler than current raw-header editing.

## Constraints

- **No Axum/reqwest types in presentation engine**: The engine must be pure policy logic in `prism_core`.
- **No new crate for v1**: Strong config/type affinity with `prism_core`; one immediate runtime consumer.
- **Executor authority preserved**: Executors remain the final authority for auth and protocol headers. Presentation output is layered under executor headers, not over them.
- **Config backward compatibility**: `headers`, `cloak`, and `claude-header-defaults` must continue to work during the migration period.

## Open Questions

- [x] Should `claude-header-defaults` be removed in v1 or kept as deprecated? -- **Decision: Deprecated with migration path. The `claude-code` profile provides built-in defaults. Operators should move custom values to per-provider `custom-headers`.**
- [x] Should the `headers` field on ProviderKeyEntry be renamed or aliased? -- **Decision: Keep `headers` as-is for backward compat. When `upstream-presentation` is set, `headers` is ignored (presentation's `custom-headers` takes precedence). When only `headers` is set, it's treated as `upstream-presentation: { profile: native, custom-headers: <headers> }`.**
- [x] Should profile activation mode support `never`? -- **Decision: No. Use `profile: native` instead. Non-native profiles support `always` (default) and `auto` (skip if real client UA detected).**

## Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|--------------------|--------|-----------|
| Module location | New crate vs `prism_core` module | `prism_core::presentation` | One consumer, strong type affinity, avoids coordination overhead |
| Profile model | Trait-plugin system vs closed enum | Closed enum (`ProfileKind`) | v1 has 4 profiles; enum is simpler, exhaustive matching, no dynamic dispatch overhead |
| Config shape | Per-profile typed structs vs flat struct with shared fields | Flat struct with profile selector | Only `claude-code` has body mutation options in v1; flat is simpler, avoids serde complexity |
| Header precedence | Profile over custom vs custom over profile | Custom overrides profile, executor overrides all | Gives operator control while preserving protocol safety |
| Legacy migration | Breaking change vs transparent normalization | Transparent normalization during config load | No configs break on upgrade |
| Body mutation ownership | Keep in `cloak.rs` vs move into presentation | Move into presentation engine | One abstraction for identity; `cloak.rs` becomes internal to presentation |
