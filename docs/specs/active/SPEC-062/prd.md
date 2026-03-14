# PRD: Vertex AI Provider

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-062       |
| Title     | Vertex AI Provider |
| Author    | AI Agent       |
| Status    | Draft          |
| Created   | 2026-03-14     |
| Updated   | 2026-03-14     |

## Problem Statement

Enterprise customers commonly use Vertex AI rather than the public Gemini API. Vertex AI uses a different URL pattern and authentication method (service account Bearer token).

## Goals

- URL construction: `https://{region}-aiplatform.googleapis.com/v1/projects/{project}/locations/{location}/publishers/google/models/{model}:generateContent`
- Authentication: OAuth2 Bearer token (SPEC-057 service account support)
- Reuse `Format::Gemini` and all Gemini translators
- Configuration: `vertex: true`, `project`, `location` fields

## Non-Goals

- Vertex AI Model Garden (non-Gemini models)
- Vertex AI Endpoints (custom deployments)

## User Stories

- As an enterprise user, I want to route requests to Vertex AI through Prism.
- As an operator, I want Vertex AI to use the same translation as Gemini.

## Success Metrics

- Vertex AI requests succeed with proper URL construction
- Service account authentication works end-to-end

## Constraints

- Depends on SPEC-057 for OAuth/service account support

## Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|--------------------|--------|-----------|
| Format | New Format::VertexAI, reuse Format::Gemini | Reuse Format::Gemini | Same API format, only URL/auth differs |
