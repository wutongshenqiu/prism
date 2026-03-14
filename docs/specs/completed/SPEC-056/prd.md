# PRD: Gemini Multimodal Enhancement

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-056       |
| Title     | Gemini Multimodal Enhancement |
| Author    | AI Agent       |
| Status    | Draft          |
| Created   | 2026-03-14     |
| Updated   | 2026-03-14     |

## Problem Statement

Two multimodal limitations:
1. Remote image URLs sent to Gemini degrade to text references `[image: url]` (openai_to_gemini.rs)
2. Gemini natively supports PDF, but Prism has no document/PDF content part translation

## Goals

- Remote image URL → Gemini `fileData` (for Gemini-supported URLs) or download to `inlineData`
- PDF/document content part → Gemini `inlineData` (`application/pdf`)
- Configurable behavior: `gemini-image-mode: file-data | download | text-fallback`

## Non-Goals

- Video content translation
- Audio content translation

## User Stories

- As a developer, I want to send image URLs in OpenAI format and have them work with Gemini.
- As a developer, I want to send PDF documents to Gemini through the proxy.

## Success Metrics

- Image URLs successfully translated to Gemini format
- PDF documents correctly sent as inlineData
- No regression in existing text-only translations

## Constraints

- Image download must respect timeout settings
- Downloaded images must respect body size limits

## Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|--------------------|--------|-----------|
| Image mode default | file-data, download, text-fallback | download | Most reliable, works with any URL |
| PDF handling | Reject, pass as text, inlineData | inlineData | Gemini supports native PDF processing |
