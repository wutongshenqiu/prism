# PRD: Codex Native Upstream & Managed OAuth

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-069       |
| Title     | Codex Native Upstream & Managed OAuth |
| Author    | AI Agent       |
| Status    | Completed      |
| Created   | 2026-03-15     |
| Updated   | 2026-03-15     |

## Problem Statement

Prism previously treated Codex as a variant of generic OpenAI API access. In practice that is the wrong contract: Codex credentials are scoped differently, the upstream base URL is different, browser automation is high-risk, and security policy must prevent subscription tokens from being sent to arbitrary OpenAI-compatible hosts.

## Goals

- Model Codex as a first-class upstream family instead of an OpenAI compatibility tweak.
- Enforce safe configuration boundaries for Codex-managed auth.
- Expose the new upstream semantics cleanly through dashboard APIs, frontend forms, and tests.
- Keep managed OAuth/runtime token material out of `config.yaml`.

## Non-Goals

- Backward-compatible parsing of legacy `openai-codex-oauth` or `openai-codex` names.
- Reverse-engineering unsupported browser automation bypasses for OAuth.

## User Stories

- As an operator, I want to create a provider with `format: openai` and `upstream: codex` so that Prism routes OpenAI-wire requests to the ChatGPT Codex backend correctly.
- As an operator, I want Codex providers to reject static API keys so that I cannot accidentally leak billing tokens or misconfigure the provider.
- As an operator, I want the dashboard to clearly distinguish OpenAI API providers from Codex subscription providers so that auth-profile choices and health checks match reality.

## Success Metrics

- Codex providers use a dedicated executor and official base URL by default.
- Invalid mixed configurations such as `upstream: codex` plus `api-key` or non-Codex auth modes are rejected with validation errors.
- Dashboard CRUD, auth-profile flows, and Playwright E2E pass with the new Codex contract.

## Constraints

- `Format` remains the ingress wire format abstraction; the new upstream dimension must not break existing OpenAI/Claude/Gemini routing.
- Managed Codex tokens remain runtime-only and must not be written back into YAML.

## Open Questions

- [ ] Whether Codex should eventually expose a dedicated health probe endpoint beyond the current compact response probe.
- [ ] Whether future work should add local Codex CLI credential import in addition to browser OAuth.

## Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|--------------------|--------|-----------|
| Codex identity model | Generic OpenAI provider, provider flag, dedicated upstream family | Dedicated upstream family | Cleanest execution, validation, and UI semantics |
| Auth mode naming | Keep `openai-codex-oauth`, rename to `codex-oauth` | `codex-oauth` | Removes misleading OpenAI API implication |
| Compatibility stance | Keep aliases, reject aliases | Reject aliases | User explicitly requested a clean design with no historical baggage |
