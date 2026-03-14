# PRD: Reverse Translation Paths

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-060       |
| Title     | Reverse Translation Paths |
| Author    | AI Agent       |
| Status    | Draft          |
| Created   | 2026-03-14     |
| Updated   | 2026-03-14     |

## Problem Statement

Prism only supports OpenAI as hub format for unidirectional translation. Sending Claude format requests cannot route to OpenAI/Gemini providers, and vice versa.

## Goals

- Claude → OpenAI request translation
- Gemini → OpenAI request translation
- OpenAI → Gemini response translation
- Claude ↔ Gemini via OpenAI intermediate format chain translation

## Non-Goals

- Direct Claude ↔ Gemini translation (always goes through OpenAI hub)

## User Stories

- As a Claude SDK user, I want my requests to work with OpenAI/Gemini providers through Prism.
- As a Gemini SDK user, I want my requests to work with Claude/OpenAI providers through Prism.

## Success Metrics

- All format pairs have working request+response translation
- No regression in existing OpenAI→Claude/Gemini paths

## Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|--------------------|--------|-----------|
| Translation topology | Star (OpenAI hub), full mesh | Star (OpenAI hub) | Simpler, reuse existing translators |
