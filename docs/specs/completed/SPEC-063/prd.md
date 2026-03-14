# SPEC-063: Unified Provider Configuration

## Problem

`Format` enum conflates two orthogonal concepts:

1. **Wire protocol** — the JSON request/response structure (OpenAI, Claude, Gemini)
2. **Provider identity** — which upstream service to call (OpenAI, DeepSeek, Vertex AI, Bailian...)

This manifests as `Format::OpenAICompat`, a fourth enum variant that is functionally identical to `Format::OpenAI` — same executor, same translation paths, always grouped together in every `match` branch. Meanwhile, Claude and Gemini already handle the same problem correctly: `ClaudeExecutor` serves both anthropic.com and Vertex AI via `base_url` detection; `GeminiExecutor` serves both Google AI and Vertex AI via the `vertex` flag. No `Format::ClaudeCompat` or `Format::GeminiCompat` exists.

The current design also forces 4 separate config arrays (`claude-api-key`, `openai-api-key`, `gemini-api-key`, `openai-compatibility`) that map 1:1 to Format variants. This prevents:

- Routing between providers sharing the same wire protocol (e.g., prefer DeepSeek over OpenAI for certain models)
- Clean provider identity in metrics, logs, and dashboard
- Adding new OpenAI-compatible providers without touching code

## Goals

1. **Purify Format** — reduce to 3 variants (OpenAI, Claude, Gemini) representing only wire protocol
2. **Unify config** — single `providers` array with explicit `name` + `format` per entry
3. **Named providers** — provider identity is a user-defined name, not derived from format
4. **Finer routing** — route planner operates on provider names, enabling per-provider policies (e.g., pin `deepseek-*` models to the `deepseek` provider)
5. **Cleaner UI** — dashboard shows 3 format options + free-text name, no artificial "OpenAI Compatible" category

## Non-Goals

- Adding new wire protocols (e.g., Bedrock native)
- Changing the translation architecture (hub-and-spoke via OpenAI format)
- Multi-credential nesting (keep flat: each entry = one credential)

## New Config Format

```yaml
providers:
  - name: openai              # unique identifier (required)
    format: openai             # wire protocol (required): openai | claude | gemini
    api-key: "sk-..."
    models: [{id: gpt-4o}, {id: gpt-4o-mini}]

  - name: openai-backup
    format: openai
    api-key: "sk-backup-..."
    weight: 1
    models: [{id: gpt-4o}]

  - name: deepseek
    format: openai
    base-url: "https://api.deepseek.com"
    api-key: "sk-ds-..."
    models: [{id: deepseek-chat}, {id: deepseek-reasoner}]

  - name: claude
    format: claude
    api-key: "sk-ant-..."

  - name: vertex-claude
    format: claude
    base-url: "https://us-central1-aiplatform.googleapis.com"
    api-key: "..."

  - name: bailian
    format: openai
    base-url: "https://coding.dashscope.aliyuncs.com/v1"
    api-key: "env://BAILIAN_API_KEY"
    models: [{id: qwen3-coder-plus}]

  - name: gemini
    format: gemini
    api-key: "..."
    models: [{id: gemini-2.5-pro}]

  - name: vertex-gemini
    format: gemini
    vertex: true
    vertex-project: my-project
    vertex-location: us-central1
    api-key: "..."
```

Key rules:
- `name` is required, must be unique across all entries
- `format` is required, one of `openai`, `claude`, `gemini`
- `base-url` defaults to the format's canonical URL if omitted
- All other fields (models, weight, region, headers, wire-api, cloak, vertex, etc.) remain unchanged

## Routing Impact

Provider pins and routing policies use provider `name` instead of format string:

```yaml
routing:
  model-resolution:
    provider-pins:
      - pattern: "deepseek-*"
        providers: ["deepseek"]
      - pattern: "gpt-*"
        providers: ["openai", "openai-backup"]
      - pattern: "claude-*"
        providers: ["claude", "vertex-claude"]

  profiles:
    default:
      provider-policy:
        strategy: ordered-fallback
        order: ["openai", "deepseek", "claude"]
        weights:
          openai: 80
          deepseek: 20
```

## Dashboard API Changes

- Provider CRUD uses `name` as the unique ID: `GET /api/dashboard/providers/{name}`
- Create request includes `format` field (replaces `provider_type`)
- List response includes `format` field per provider
- No more `"openai-compat"` in any API surface

## Dashboard UI Changes

- Provider Type dropdown becomes Format dropdown (3 options: OpenAI, Claude, Gemini)
- Name field is always visible and required (free-text, unique)
- Base URL field always visible with format-specific placeholder
- Wire API field shown when format = openai (not hidden behind a specific type)
- Provider table shows Name and Format as separate columns

## Metrics & Logs

- `provider` field in logs/metrics shows the provider `name` (e.g., "deepseek") instead of format string
- Provider distribution chart uses provider names for finer granularity
