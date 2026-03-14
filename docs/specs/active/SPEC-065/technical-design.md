# Technical Design: Canonical Multi-Protocol Gateway & Control Plane Redesign

| Field     | Value                                              |
|-----------|----------------------------------------------------|
| Spec ID   | SPEC-065                                           |
| Title     | Canonical Multi-Protocol Gateway & Control Plane Redesign |
| Author    | AI Agent                                           |
| Status    | Draft                                              |
| Created   | 2026-03-14                                         |
| Updated   | 2026-03-14                                         |
| Parent    | #211                                               |
| Issues    | #212, #213, #214, #215, #216, #217, #218, #219, #220, #221, #222, #223, #224, #225, #226 |

## Overview

This redesign makes OpenAI, Claude, and Gemini first-class ingress protocols while removing protocol bias from the runtime. The new architecture is adapter-core-adapter:

1. Public protocol adapters parse HTTP requests into canonical runtime types.
2. The runtime plans and executes using canonical request semantics and provider/model capability declarations.
3. Provider executors speak native upstream protocols.
4. Egress adapters translate canonical results back into the client's public protocol.

The control plane is redesigned around protocol support, provider capabilities, routing explanation, replay, and schema-driven configuration instead of raw provider-centric forms.

Reference: [PRD](prd.md)

## API Design

Public endpoints remain tri-protocol, but all inference endpoints share one runtime path:

### Public Ingress

```text
GET  /v1/models
POST /v1/chat/completions
POST /v1/responses

POST /v1/messages
POST /v1/messages/count_tokens

GET  /v1beta/models
POST /v1beta/models/{model}:generateContent
POST /v1beta/models/{model}:streamGenerateContent
```

### Control Plane APIs

New or reshaped dashboard APIs:

```text
GET  /api/dashboard/protocols/matrix
GET  /api/dashboard/providers/capabilities
POST /api/dashboard/routing/explain
POST /api/dashboard/replay/preview
POST /api/dashboard/replay/execute
GET  /api/dashboard/config/schema
```

### Canonical Explain Request

```json
{
  "ingress_protocol": "claude",
  "operation": "generate",
  "endpoint": "messages",
  "model": "claude-sonnet-4-5",
  "stream": true,
  "features": {
    "tools": true,
    "json_schema": false,
    "reasoning": true,
    "images": false,
    "count_tokens": false
  },
  "tenant_id": "tenant-a",
  "api_key_id": "sk-proxy-abc",
  "region": "us-east"
}
```

### Canonical Explain Response

```json
{
  "selected": {
    "provider": "anthropic-prod",
    "credential": "anthropic-us-1",
    "model": "claude-sonnet-4-5",
    "execution_mode": "native"
  },
  "alternates": [],
  "rejections": [
    {
      "candidate": "openai-prod/gpt-5",
      "reason": "missing_capability: messages_protocol"
    }
  ],
  "required_capabilities": {
    "supports_stream": true,
    "supports_tools": true,
    "supports_reasoning": true
  }
}
```

## Backend Implementation

### Module Structure

```text
crates/
├── domain/
│   ├── request.rs
│   ├── response.rs
│   ├── events.rs
│   ├── capability.rs
│   └── operation.rs
├── protocol-openai/
├── protocol-claude/
├── protocol-gemini/
├── capabilities/
├── router/
├── runtime/
├── providers-openai/
├── providers-anthropic/
├── providers-gemini/
└── control-plane-api/
```

If crate extraction is deferred initially, the same boundaries should still exist as top-level modules with the same ownership.

### Key Types

```rust
pub enum IngressProtocol {
    OpenAi,
    Claude,
    Gemini,
}

pub enum Operation {
    Models,
    Generate,
    CountTokens,
}

pub struct CanonicalRequest {
    pub ingress_protocol: IngressProtocol,
    pub operation: Operation,
    pub endpoint: String,
    pub model: String,
    pub stream: bool,
    pub input: Conversation,
    pub tools: Vec<ToolSpec>,
    pub tool_choice: ToolChoice,
    pub response_format: ResponseFormat,
    pub reasoning: Option<ReasoningConfig>,
    pub attachments: Vec<Attachment>,
    pub limits: RequestLimits,
    pub tenant_id: Option<String>,
    pub api_key_id: Option<String>,
    pub region: Option<String>,
}

pub struct RequiredCapabilities {
    pub supports_generate: bool,
    pub supports_messages_protocol: bool,
    pub supports_responses_protocol: bool,
    pub supports_gemini_generate_content: bool,
    pub supports_stream: bool,
    pub supports_tools: bool,
    pub supports_parallel_tools: bool,
    pub supports_json_schema: bool,
    pub supports_reasoning: bool,
    pub supports_images: bool,
    pub supports_count_tokens: bool,
}

pub enum ExecutionMode {
    Native,
    LosslessAdapted,
}
```

### Runtime Flow

```text
HTTP request
  -> ingress adapter
  -> CanonicalRequest
  -> capability derivation
  -> planner
  -> selected provider/model/credential
  -> provider-native request builder
  -> upstream executor
  -> CanonicalResponse / CanonicalEvent stream
  -> egress adapter
  -> HTTP response
```

### Capability Model

Capability declarations are first-class and explicit:

- provider-level defaults
- model-level overrides
- operation-level support
- protocol-specific ingress/egress support

The planner filters candidates by capability compatibility before any cost or latency scoring.

### Routing Rules

Routing input must include:

- ingress protocol
- operation
- endpoint semantics
- stream flag
- required capabilities
- tenant
- api key
- region
- model selector

The planner result includes:

- selected route
- execution mode
- capability mismatches
- filtered candidates
- ranked alternates

### Public Endpoint Unification

No public inference handler may bypass the runtime. In particular:

- `responses` is not a special direct path
- `messages` is not allowed to rely on hidden Claude-only assumptions
- `Gemini generateContent` is not mapped to a lossy fallback path implicitly

All explain, preview, replay, and runtime execution must use the same planner and capability filter.

## Configuration Changes

The control plane and runtime config should move from provider-format-centric fields to schema-driven capability and protocol declarations.

### Provider Config Shape

```yaml
providers:
  - name: anthropic-prod
    upstream:
      type: anthropic
      credential_ref: env://ANTHROPIC_API_KEY
      base_url: https://api.anthropic.com
    models:
      - id: claude-sonnet-4-5
        aliases: [claude-default]
        capabilities:
          supports_stream: true
          supports_tools: true
          supports_reasoning: true
          supports_count_tokens: true
```

### Public Protocol Config

```yaml
public_protocols:
  openai:
    enabled: true
  claude:
    enabled: true
  gemini:
    enabled: true
```

### Routing Config

```yaml
routing:
  profiles:
    balanced:
      strategy: weighted-round-robin
  rules:
    - name: claude-reasoning
      match:
        ingress_protocols: [claude]
        capabilities: [supports_reasoning]
      use_profile: balanced
```

## Provider Compatibility

| Provider Type | Ingress Protocols It Can Back | Execution Mode | Notes |
|---------------|-------------------------------|----------------|-------|
| OpenAI-compatible | OpenAI, Claude, Gemini | Native or LosslessAdapted | Depends on declared capabilities |
| Anthropic Claude | OpenAI, Claude, Gemini | Native or LosslessAdapted | Native for Claude protocol and count tokens |
| Gemini / Vertex | OpenAI, Claude, Gemini | Native or LosslessAdapted | Native for Gemini generateContent |

## UI Design

### Information Architecture

```text
Overview
Protocols
Providers
Models & Capabilities
Routing
Requests
Replay
Tenants & Keys
Config & Changes
```

### Page Responsibilities

- `Protocols`: public protocol matrix, endpoint semantics, supported execution modes.
- `Providers`: upstream connectivity, health, coverage, capability declarations.
- `Models & Capabilities`: provider/model capability matrix with filters and diff-friendly views.
- `Routing`: real planner explain, candidate ranking, rejection reasons, execution mode.
- `Requests`: request log plus canonical request summary and route trace.
- `Replay`: replay saved or synthetic requests through explain or execution mode.
- `Config & Changes`: schema-driven editor, validation, diff, apply history.

### UI Principles

- Protocol-first, not raw-format-first.
- Capability-first, not model-list-first.
- Explain views must use the same backend logic as runtime.
- Every request detail should expose ingress protocol, canonical operation, selected provider, execution mode, and rejection reasons.
- Complex edits should be form/schema-driven, not large free-text config blobs by default.

## Alternative Approaches

| Approach | Pros | Cons | Verdict |
|----------|------|------|---------|
| Keep OpenAI-centric core and patch other protocols | Lower immediate rewrite cost | Continues protocol bias and hidden edge cases | Rejected |
| Protocol-to-protocol translators only | Simple mental model at first | Becomes brittle, lossy, and hard to reason about | Rejected |
| Single public OpenAI protocol only | Simplest runtime | Does not match product goals | Rejected |
| Canonical IR with tri-protocol ingress | Clear semantics and scalable architecture | Requires larger upfront reorganization | Chosen |

## Task Breakdown

- [ ] Define canonical domain types for requests, responses, events, operations, and capabilities.
- [ ] Build capability registry and planner v2 on canonical request semantics.
- [ ] Add OpenAI ingress and egress adapters using canonical runtime types.
- [ ] Add Claude ingress and egress adapters using canonical runtime types.
- [ ] Add Gemini ingress and egress adapters using canonical runtime types.
- [ ] Rewrite provider executors to consume canonical runtime contracts and emit canonical results.
- [ ] Unify all public inference handlers on one runtime pipeline.
- [ ] Add control-plane APIs for protocol matrix, capabilities, route explain, and replay.
- [ ] Redesign dashboard navigation and page information architecture.
- [ ] Build protocol matrix and provider capability pages in the dashboard.
- [ ] Rebuild routing explain, request inspection, and replay UX.
- [ ] Add golden fixtures, planner contract tests, provider contract tests, and end-to-end protocol matrix tests.

## Test Strategy

- **Unit tests:** canonical type invariants, capability derivation, planner filtering, protocol adapters, provider-native builders.
- **Golden tests:** protocol request and response fixtures for OpenAI, Claude, and Gemini.
- **Planner contract tests:** route explanation snapshots for combinations of capabilities and routing rules.
- **Provider contract tests:** mocked upstream compatibility for OpenAI-compatible, Anthropic, and Gemini executors.
- **Integration tests:** end-to-end inference for each public protocol through the unified runtime pipeline.
- **Dashboard tests:** route explain page, protocol matrix page, provider capability page, and replay flow.

## Rollout Plan

1. Land canonical domain and planner infrastructure behind new modules.
2. Port each public protocol adapter and provider executor in focused issues.
3. Switch public handlers and explain APIs to the unified runtime.
4. Rebuild dashboard pages on top of the new APIs.
5. Remove obsolete endpoint-local logic once all tests pass under the new runtime.
