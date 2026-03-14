# Technical Design: Reverse Translation Paths

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-060       |
| Title     | Reverse Translation Paths |
| Author    | AI Agent       |
| Status    | Draft          |
| Created   | 2026-03-14     |
| Updated   | 2026-03-14     |

## Overview

Adds reverse translation paths to enable Claude and Gemini format requests to be routed to any provider. Uses OpenAI as the hub format for all translations.

## Backend Implementation

### New Translator Files

1. `claude_to_openai_request.rs` — Claude Messages → OpenAI Chat Completions request
2. `gemini_to_openai_request.rs` — Gemini generateContent → OpenAI Chat Completions request (may merge with SPEC-055)
3. `openai_to_gemini_response.rs` — OpenAI response → Gemini generateContent response (may merge with SPEC-055)

### Translation Matrix (after implementation)

| Source → Target | OpenAI | Claude | Gemini |
|----------------|--------|--------|--------|
| OpenAI         | pass   | ✓ existing | ✓ existing |
| Claude         | ✓ NEW  | pass   | via OpenAI |
| Gemini         | ✓ NEW  | via OpenAI | pass |

## Task Breakdown

- [ ] Create claude_to_openai_request.rs
- [ ] Create gemini_to_openai_request.rs (or extend from SPEC-055)
- [ ] Create openai_to_gemini_response.rs (or extend from SPEC-055)
- [ ] Register all new translation pairs in registry
- [ ] Unit tests for each translator
- [ ] Integration tests for cross-format routing

## Test Strategy

- **Unit tests:** Each translation function independently
- **Integration tests:** Claude request → OpenAI provider, Gemini request → Claude provider
