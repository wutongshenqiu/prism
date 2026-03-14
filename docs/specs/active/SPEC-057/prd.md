# PRD: OAuth & Auth-File Provider Authentication

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-057       |
| Title     | OAuth & Auth-File Provider Authentication |
| Author    | AI Agent       |
| Status    | Draft          |
| Created   | 2026-03-14     |
| Updated   | 2026-03-14     |

## Problem Statement

Modern CLI tools (Claude Code, Codex, Gemini CLI) authenticate via OAuth rather than API keys. Prism currently only supports static API keys as upstream credential sources, preventing integration with OAuth-authenticated providers.

## Goals

- Auth-file credential source: read CLI tool local credential files (~/.claude/credentials.json etc.)
- OAuth2 client-credentials flow for Vertex AI etc.
- GCP service account: read JSON key → JWT → access token
- File-watch auto-refresh (reuse notify crate)
- Background token refresh loop, transparent to dispatch
- Dashboard UI support for new credential types

## Non-Goals

- Browser-popup OAuth authorization (users complete OAuth via CLI themselves)
- Built-in OAuth server callback handler

## User Stories

- As an operator, I want to use my existing Claude CLI credentials with Prism.
- As an enterprise user, I want to use GCP service account keys for Vertex AI.
- As an operator, I want credentials to auto-refresh without manual intervention.

## Success Metrics

- Auth-file credentials work with Claude CLI credentials
- OAuth2 tokens refresh automatically before expiry
- Service account JWT flow produces valid access tokens

## Constraints

- Must not break existing static API key configuration
- Token refresh must be non-blocking to dispatch
- Supersedes SPEC-047

## Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|--------------------|--------|-----------|
| Credential abstraction | Trait object, enum, strategy | Enum (CredentialSource) | Simple, covers known variants |
| Refresh mechanism | On-demand, background timer, file watch | Background timer + file watch | Proactive, no latency impact |
