# PRD: Managed Auth Profiles & Claude Subscription Tokens

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-068       |
| Title     | Managed Auth Profiles & Claude Subscription Tokens |
| Author    | AI Agent       |
| Status    | Completed      |
| Created   | 2026-03-15     |
| Updated   | 2026-03-15     |

## Problem Statement

Prism currently treats upstream auth profiles as either static secrets or a single hard-coded `openai-codex-oauth` flow. That is too narrow for modern coding-agent workflows and leaves Claude subscription credentials without a first-class path. It also makes provider-specific security controls difficult because managed credentials are not modeled consistently.

## Goals

- Introduce a clean managed-auth model for runtime-managed upstream credentials.
- Add first-class Claude subscription token support using `claude setup-token` style credentials.
- Keep managed credentials out of `config.yaml` and persist them only in the runtime sidecar store.
- Enforce provider-specific safety controls for sensitive managed credentials.
- Tighten upstream header protection so auth headers cannot be overridden by forwarded/custom headers.

## Non-Goals

- Reverse-engineering or depending on undocumented Claude browser OAuth login flows.
- Supporting arbitrary third-party base URLs for Claude subscription tokens.
- Building a general OAuth plugin SDK in this change.

## User Stories

- As an operator, I want to connect a Claude subscription credential in the dashboard without storing it in config.
- As an operator, I want Codex OAuth and Claude subscription auth to follow one managed-auth model in the backend.
- As a security-conscious deployer, I want Prism to reject dangerous combinations like Claude subscription tokens on non-Anthropic upstream hosts.

## Success Metrics

- Claude managed auth profiles can be created, connected, listed, and used for routing.
- Managed auth state is written only to the auth runtime sidecar file.
- Upstream protected auth headers cannot be overridden by request or profile headers.
- Codex OAuth behavior continues to function after the refactor.

## Constraints

- Dashboard APIs and UI must remain simple enough for operators to reason about.
- Runtime-managed credentials must survive config reloads and provider CRUD operations.
- The design must be extensible to future managed auth providers without another full rewrite.

## Open Questions

- [ ] Whether Anthropic will eventually publish a stable browser OAuth flow suitable for gateways.
- [ ] Whether Claude subscription credentials should gain an optional lightweight upstream verification endpoint in a later iteration.

## Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|--------------------|--------|-----------|
| Claude subscription support shape | Browser OAuth, CLI credential scraping, setup-token style managed token | Setup-token style managed token | Stable operational model, lower fragility, lower security risk |
| Managed credential persistence | Config file, sidecar runtime store, external secret backend only | Sidecar runtime store | Keeps secrets out of config while preserving hot reload and dashboard UX |
| Claude host safety | Allow any base URL, warn only, restrict to official Anthropic host | Restrict to official Anthropic host | Reduces accidental exfiltration of high-value personal subscription credentials |
| Upstream auth header handling | Preserve current overwrite behavior, warn only, hard-block protected headers | Hard-block protected headers | Prevents request/custom header injection from replacing upstream auth |
