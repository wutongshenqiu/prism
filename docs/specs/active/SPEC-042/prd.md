# PRD: Crate Structure Refactoring

| Field     | Value                        |
|-----------|------------------------------|
| Spec ID   | SPEC-042                     |
| Title     | Crate Structure Refactoring  |
| Author    | AI Agent                     |
| Status    | Draft                        |
| Created   | 2026-03-12                   |
| Updated   | 2026-03-12                   |

## Problem Statement

`prism-core` is a "god crate" containing 22 public modules spanning completely unrelated domains: provider trait definitions, YAML config loading, process daemonization, systemd integration, response caching, rate limiting, metrics, Prometheus export, HTTP proxy building, circuit breakers, and more. This causes:

1. **Dependency pollution**: Any crate that needs just the `Format` enum or `ProviderExecutor` trait is forced to transitively depend on `moka`, `reqwest`, `notify`, `fork`, `sd-notify`, `libc`, `tracing-appender`, etc.
2. **Slow incremental compilation**: Changing any module in core invalidates all downstream crates (`provider`, `translator`, `server`), even when the change is unrelated.
3. **Poor reusability**: External projects cannot use `prism-translator` or `prism-provider` without pulling in the entire core dependency tree.
4. **Zombie dependencies**: `jsonwebtoken` and `bcrypt` are declared in core's `Cargo.toml` but used nowhere in core — they're only used in `prism-server`.
5. **Binary bloat**: TLS serving logic (`rustls`, `hyper`, `hyper-util`) lives in the binary entry point instead of `prism-server`, forcing the binary crate to depend on low-level HTTP/TLS crates.
6. **Code duplication**: `prism-server` reimplements HTTP client building (`reqwest`) instead of reusing `prism-core::proxy`.

## Goals

- Extract a lightweight `prism-types` crate containing only shared types and traits with minimal dependencies
- Remove zombie dependencies (`jsonwebtoken`, `bcrypt`) from `prism-core`
- Eliminate code duplication between `prism-core::proxy` and `prism-server::handler::dashboard::providers`
- Extract `lifecycle/` modules from core into a dedicated `prism-lifecycle` crate
- Move TLS/HTTP serving logic from binary `src/app.rs` into `prism-server`
- Remove `axum` dependency from `prism-core`
- Reduce incremental rebuild scope for translator and provider crates

## Non-Goals

- Changing any user-facing behavior, API endpoints, or configuration format
- Adding new features or capabilities
- Refactoring individual module internals (e.g., splitting `memory_log_store.rs` into sub-modules)
- Modifying the web dashboard frontend

## User Stories

- As a contributor, I want to modify the rate limiter without triggering a full workspace rebuild so that iteration is faster.
- As an external developer, I want to depend on `prism-types` to integrate with Prism's type system without pulling in 30+ transitive dependencies.
- As a maintainer, I want each crate to have a clear, bounded responsibility so that code review is scoped.

## Success Metrics

- `prism-types` has zero "heavy" dependencies (no `moka`, `reqwest`, `notify`, `fork`, `sd-notify`, `axum`, `tracing-appender`)
- `prism-translator` depends only on `prism-types` (not `prism-core`)
- `prism-provider` depends only on `prism-types` (not `prism-core`)
- `prism-core` Cargo.toml has no `jsonwebtoken`, `bcrypt`, or `axum` dependency
- Binary `Cargo.toml` has no `rustls`, `hyper`, `hyper-util`, `tower` dependencies
- All existing tests pass with zero regression
- `cargo clippy --workspace --tests -- -D warnings` passes
- `cargo fmt --check` passes

## Constraints

- Pure refactor: zero behavior change across all API endpoints
- All 4 phases can be landed as separate PRs to reduce review burden
- Each phase must independently pass `make lint && make test`
- Maintain backward-compatible re-exports where needed during migration

## Open Questions

- [x] Should `prism-types` include `request_log.rs` trait? — Yes, `LogStore` trait is needed by both server and core
- [x] Are `jsonwebtoken` and `bcrypt` truly unused in core? — Confirmed: zero imports in any core module
- [ ] Should `prism-lifecycle` be a separate crate or a feature-gated module in core? — Separate crate preferred for clean unix isolation

## Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|--------------------|--------|-----------|
| Type extraction | Feature flags in core vs. new crate | New `prism-types` crate | Feature flags add complexity; separate crate is idiomatic Rust (cf. axum-core, tower-layer) |
| Lifecycle extraction | Keep in core vs. new crate | New `prism-lifecycle` crate | Unix-specific deps (fork, libc, sd-notify) shouldn't pollute core |
| Error StatusCode | Keep `axum::StatusCode` vs. use `u16` | Use `u16` in types, convert in server | Removes axum dependency from types/core layer |
| Migration strategy | Big bang vs. phased | Phased (4 PRs) | Each phase is independently testable and reviewable |
