# PRD: Coding Agent Compatibility Endpoints

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-044       |
| Title     | Coding Agent Compatibility Endpoints |
| Author    | Claude          |
| Status    | Active         |
| Created   | 2026-03-13     |
| Updated   | 2026-03-13     |

## Problem Statement

Prism's compatibility surface is narrower than needed for drop-in adoption by coding agents and SDKs. Missing endpoints cause connection failures.

## Goals

- Add `/v1/completions` endpoint (legacy OpenAI completions API)
- Add `/v1/messages/count_tokens` passthrough for Anthropic token counting
- Integrate with existing auth, routing, and observability

## Non-Goals

- WebSocket-based `/v1/responses` streaming (deferred — complex, low demand)
- Full legacy completions API semantics (just proxy to chat format)

## Design Decisions

| Decision | Options | Chosen | Rationale |
|----------|---------|--------|-----------|
| `/v1/completions` impl | Full legacy support, proxy as chat | Proxy as OpenAI format | All providers handle chat; legacy completions is deprecated |
| `count_tokens` | Implement locally, proxy to upstream | Proxy to upstream | Accurate; avoids maintaining tokenizer |
