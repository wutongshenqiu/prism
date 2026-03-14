# PRD: Thinking Signature Cache

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-053       |
| Title     | Thinking Signature Cache |
| Author    | AI Agent       |
| Status    | Draft          |
| Created   | 2026-03-14     |
| Updated   | 2026-03-14     |

## Problem Statement

Claude extended thinking generates `thinking` content blocks with a `signature` field. In multi-turn conversations, clients must send back previous thinking blocks with their signatures. When Prism routes different turns to different credentials, signatures become invalid, breaking the conversation. This is the most critical functionality gap — any user using Claude thinking models through the proxy is affected.

## Goals

- Implement thinking content → signature in-memory cache (SHA256 hash key, 3h TTL)
- Auto-extract signatures from responses in the dispatch layer
- Auto-inject cached signatures for thinking blocks missing signatures in requests
- Multi-tenant isolation (cache key includes tenant_id + model)
- Configurable TTL, max entries, enable/disable

## Non-Goals

- Persist signatures to disk (in-memory cache is sufficient)
- Share signatures across instances (single-node scenario)

## User Stories

- As a developer using Claude thinking models through Prism, I want my multi-turn conversations to work correctly even when routed to different credentials, so that thinking signatures remain valid.
- As an operator, I want to configure the thinking cache TTL and max entries to manage memory usage.

## Success Metrics

- Multi-turn thinking conversations work correctly with credential rotation
- Cache hit rate > 90% for multi-turn conversations
- Memory usage stays bounded by max-entries config

## Constraints

- Must use moka crate (already in dependencies for response cache)
- Must not introduce significant latency on the request/response path

## Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|--------------------|--------|-----------|
| Cache backend | moka, dashmap+manual eviction, LRU | moka | Already in deps, async-friendly, TTL built-in |
| Cache key | SHA256(text), text itself, hash(text+model) | (tenant_id, model, SHA256(text)) | Multi-tenant isolation, compact key |
| Integration point | middleware, handler, dispatch | dispatch/executor.rs | Closest to where requests/responses are processed |
