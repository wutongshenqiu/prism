# PRD: Configuration System & Hot-Reload

| Field     | Value                              |
|-----------|------------------------------------|
| Spec ID   | SPEC-004                           |
| Title     | Configuration System & Hot-Reload  |
| Author    | AI Proxy Team                      |
| Status    | Completed                          |
| Created   | 2026-02-27                         |
| Updated   | 2026-02-27                         |

## Problem Statement

AI Proxy Gateway requires a flexible configuration system that allows operators to manage provider credentials, routing strategies, security settings, and operational parameters without restarting the service. Downtime during credential rotation or tuning is unacceptable for a production API gateway.

## Goals

- YAML-based configuration file (`config.yaml`) with kebab-case field naming
- File-watching with hot-reload: detect config file changes and apply them at runtime without restarts
- ArcSwap-based atomic swap: readers are never blocked while configuration is being replaced
- Environment variable overrides via `dotenvy` and clap `env` attributes
- CLI parameter overrides for host, port, config path, and log level
- Validation of configuration on load (TLS settings, proxy URLs)
- Sanitization of provider key entries (dedup, normalize base URLs, lowercase headers)

## Non-Goals

- Remote configuration sources (e.g., etcd, Consul) -- file-based only
- Per-request configuration overrides via headers
- Configuration versioning or rollback history

## User Stories

- As an operator, I want to rotate API keys by editing `config.yaml` so that new credentials take effect without restarting the service.
- As an operator, I want to override the listen port via `PRISM_PORT` environment variable so that I can configure it per-environment.
- As an operator, I want invalid config changes to be rejected so that a typo does not take down the proxy.
- As a developer, I want CLI flags (`--host`, `--port`, `--log-level`) to override config file values so that I can quickly test different settings.

## Success Metrics

- Config reload completes in under 200ms with zero dropped requests
- Invalid config files are rejected with clear error messages; the previous valid config remains active
- SHA256 deduplication prevents redundant reloads when file content has not changed

## Constraints

- Must use `serde_yml` for YAML deserialization with kebab-case renaming
- Must use `arc-swap` crate for lock-free atomic config replacement
- Must use `notify` crate for cross-platform file watching
- Config file path defaults to `config.yaml`, overridable via `-c`/`--config` flag or `PRISM_CONFIG` env var

## Open Questions

- [x] Should config reload trigger credential router rebuild? -- Yes, `update_from_config` is called on each reload

## Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|--------------------|--------|-----------|
| Config storage | Mutex, RwLock, ArcSwap | ArcSwap | Lock-free reads; writers never block readers |
| File watching | Polling, inotify/kqueue | notify crate | Cross-platform, event-driven, low overhead |
| Dedup mechanism | Timestamp, hash | SHA256 hash | Content-based; avoids spurious reloads from touch/save-without-changes |
| Debounce interval | 50ms, 150ms, 500ms | 150ms | Balances responsiveness with batching rapid saves |
