# PRD: Gemini Native API Endpoints

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-055       |
| Title     | Gemini Native API Endpoints |
| Author    | AI Agent       |
| Status    | Draft          |
| Created   | 2026-03-14     |
| Updated   | 2026-03-14     |

## Problem Statement

Gemini CLI and Gemini SDK use native endpoint formats (`/v1beta/models/{model}:generateContent`). Prism currently does not expose these endpoints, making it impossible to serve as a drop-in proxy for Gemini CLI/SDK.

## Goals

- Standard endpoints: `POST /v1beta/models/{model}:generateContent` (non-streaming)
- Standard endpoints: `POST /v1beta/models/{model}:streamGenerateContent` (streaming SSE)
- Model listing: `GET /v1beta/models` (Gemini format)
- Internal endpoints: `POST /v1internal:generateContent` (Gemini CLI)
- Internal endpoints: `POST /v1internal:streamGenerateContent` (Gemini CLI)
- Auth: `x-goog-api-key` header or query param `?key=...`
- Cross-provider routing: Gemini entry can route to Claude/OpenAI providers

## Non-Goals

- Gemini Files API (`/v1beta/files`)
- Gemini Tuning API
- Gemini Caching API

## User Stories

- As a Gemini CLI user, I want to point my CLI at Prism and have it work transparently.
- As a Gemini SDK user, I want to use my existing code with Prism as the base URL.
- As an operator, I want Gemini format requests to be routed to any provider (Claude, OpenAI, Gemini).

## Success Metrics

- Gemini CLI connects and completes requests successfully through Prism
- Cross-provider routing works for Gemini format requests
- v1internal endpoints restricted to localhost

## Constraints

- v1internal endpoints must only be accessible from localhost
- Must maintain compatibility with existing OpenAI/Claude endpoints

## Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|--------------------|--------|-----------|
| Model extraction | From body, from URL path | URL path | Gemini protocol puts model in URL |
| v1internal security | Auth required, localhost-only, both | Localhost-only | Matches Gemini CLI behavior |
