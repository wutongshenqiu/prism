# PRD: Structured Output Translation

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-059       |
| Title     | Structured Output Translation |
| Author    | AI Agent       |
| Status    | Draft          |
| Created   | 2026-03-14     |
| Updated   | 2026-03-14     |

## Problem Statement

`response_format` with `json_schema` is currently only passed through, not translated between providers. Structured output requests fail when routed cross-provider.

## Goals

- OpenAI `json_schema` → Claude: synthetic tool + `tool_choice` forced call, unwrap tool params in response
- OpenAI `json_schema` → Gemini: `responseMimeType` + `responseSchema`
- OpenAI `json_object` → Claude: system prompt JSON instruction injection
- OpenAI `json_object` → Gemini: `responseMimeType: "application/json"`

## Non-Goals

- Arbitrary JSON schema validation
- Schema transformation between incompatible types

## User Stories

- As a developer, I want structured output to work when my request is routed to Claude.
- As a developer, I want JSON schema constraints to work with Gemini.

## Success Metrics

- Structured output requests produce valid JSON across all providers
- Schema constraints are respected in translations

## Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|--------------------|--------|-----------|
| Claude approach | System prompt, synthetic tool, native (future) | Synthetic tool | Most reliable, forced output |
| Gemini approach | System prompt, responseSchema | responseSchema | Native Gemini support |
