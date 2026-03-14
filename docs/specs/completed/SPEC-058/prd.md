# PRD: Provider-Scoped Routing & Amp Integration

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-058       |
| Title     | Provider-Scoped Routing & Amp Integration |
| Author    | AI Agent       |
| Status    | Draft          |
| Created   | 2026-03-14     |
| Updated   | 2026-03-14     |

## Problem Statement

Amp and similar CLI tools need provider-scoped API paths (`/api/provider/{provider}/v1/chat/completions`), allowing clients to specify routing to a specific provider rather than relying on automatic routing.

## Goals

- Provider-scoped endpoints: `/api/provider/{provider}/v1/chat/completions`
- Provider-scoped endpoints: `/api/provider/{provider}/v1/messages`
- `{provider}` maps to credential name or format name
- Reuse existing dispatch logic with `allowed_credentials` constraint

## Non-Goals

- Amp-specific OAuth/key management UI
- Amp config auto-discovery

## User Stories

- As an Amp user, I want to specify which provider to use for each request.
- As an operator, I want provider-scoped routing that reuses existing infrastructure.

## Success Metrics

- Amp CLI works with provider-scoped endpoints
- Requests correctly routed to specified provider

## Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|--------------------|--------|-----------|
| Provider matching | Name only, format only, both | Both (name first, then format) | Flexible, covers all use cases |
