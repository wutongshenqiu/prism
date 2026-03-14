# PRD: OAuth & Auth-File Upstream Onboarding

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-047       |
| Title     | OAuth & Auth-File Upstream Onboarding |
| Author    | Claude          |
| Status    | Draft          |
| Created   | 2026-03-13     |
| Updated   | 2026-03-13     |

## Problem Statement

Prism currently only supports static API keys for upstream provider credentials. Some providers and adoption scenarios require OAuth-driven or auth-file based credential onboarding, where tokens are refreshed automatically or read from local credential files managed by external tools.

## Goals

- Support auth-file based credentials (e.g. `~/.config/provider/credentials.json`) that are watched and reloaded
- Support OAuth2 client-credentials flow for providers that require it
- Automatic token refresh before expiry
- Integrate refreshed credentials into existing routing, rotation, and auditing
- Dashboard UX for managing OAuth-backed accounts

## Non-Goals

- Browser-based OAuth authorization code flow (deferred)
- User-facing multi-tenant OAuth (this is operator-side)

## Open Questions

1. Which providers require OAuth vs auth-file? (Need research)
2. Where to persist OAuth tokens? (File, in-memory, SQLite?)
3. How does token refresh interact with circuit breaker state?
4. Should auth-file watching reuse the existing ConfigWatcher or have its own watcher?

## Design Decisions

_To be filled in during technical design phase._

## References

- GitHub Issue #155
- GitHub Epic #157
- `config.example.yaml` from CLIProxyAPI
