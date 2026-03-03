# PRD: Translator Unit Tests

| Field     | Value                    |
|-----------|--------------------------|
| Spec ID   | SPEC-029                 |
| Title     | Translator Unit Tests    |
| Author    | Claude                   |
| Status    | Draft                    |
| Created   | 2026-03-03               |
| Updated   | 2026-03-03               |

## Problem Statement

The translator crate (`crates/translator/`) has **zero tests** despite being a critical component that handles format translation between OpenAI, Claude, and Gemini APIs. Any regression in translation logic could silently corrupt requests/responses across all providers, making this a high-risk untested area.

## Goals

- Add comprehensive unit tests for all 4 translator modules (openai_to_claude, claude_to_openai, openai_to_gemini, gemini_to_openai)
- Add unit tests for the TranslatorRegistry (lib.rs)
- Add roundtrip integration tests (OpenAI → Claude → OpenAI, OpenAI → Gemini → OpenAI)
- Extract shared helpers to reduce duplication across translator modules
- Achieve ~56 tests covering all translation paths

## Non-Goals

- Modifying existing translation logic (pure test addition)
- Adding new translation paths
- E2E testing with real API calls (covered by SPEC-033)

## User Stories

- As a developer, I want translator unit tests so that I can safely refactor translation logic without fear of breaking cross-provider compatibility.
- As a CI pipeline, I want automated translator tests so that PRs with broken translations are caught before merge.

## Success Metrics

- ~56 translator tests passing
- All edge cases covered: empty content, tool calls, images, streaming events, finish reasons
- Roundtrip tests verify format preservation

## Constraints

- Tests must run without network access (pure unit tests)
- No changes to public API surface
- Must pass `make lint` and `make test`

## Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|--------------------|--------|-----------|
| Test framework | built-in #[test], test-case | test-case for parameterized | Reduces boilerplate for similar test scenarios |
| JSON comparison | manual asserts, assert_json_diff | assert_json_diff | Better error messages for JSON mismatches |
| Snapshot testing | insta, manual | Not used | Translation outputs are deterministic, direct asserts are clearer |
