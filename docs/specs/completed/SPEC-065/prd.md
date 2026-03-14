# PRD: Canonical Multi-Protocol Gateway & Control Plane Redesign

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

## Problem Statement

Prism currently exposes multiple public protocols, but the implementation is not protocol-native end to end. The runtime is effectively centered on OpenAI Chat semantics, with Claude and Gemini support layered on through route-specific handling and protocol-to-protocol translation.

This creates four structural problems:

1. Public protocol support is not modeled as a first-class runtime concept.
2. Routing is based mostly on model availability, not on request semantics and provider capabilities.
3. Some endpoints bypass the main routing path, making preview, explain, retry, and execution behavior inconsistent.
4. The dashboard is config-centric and provider-centric, but not protocol-centric or capability-centric, so it does not accurately explain what the gateway can do.

The result is a system that can appear to support OpenAI, Claude, and Gemini equally, while the actual execution model is uneven and difficult to reason about.

## Goals

1. Support OpenAI, Claude, and Gemini as first-class public protocols.
2. Replace protocol-to-protocol runtime chaining with a canonical internal request, response, and event model.
3. Route requests based on explicit request semantics and provider/model capabilities, not just model strings.
4. Ensure all public inference endpoints use one runtime pipeline for planning, explanation, retries, logging, metrics, and execution.
5. Redesign the dashboard so it explains protocols, capabilities, routing, and runtime behavior clearly.
6. Make the implementation easy to change in small, testable increments by Claude Code and similar coding agents.

## Non-Goals

- Backward compatibility with the current internal architecture.
- Incremental migration constraints or compatibility shims for the old dispatch model.
- Supporting every provider-specific edge case in v1 if it cannot be represented losslessly.
- Preserving existing dashboard information architecture.

## User Stories

- As a gateway operator, I want to expose OpenAI, Claude, and Gemini APIs from one gateway and know that each public protocol is handled consistently.
- As a gateway operator, I want routing decisions to account for tools, JSON schema, reasoning, multimodal input, streaming, and token counting support.
- As a dashboard user, I want to see which protocols and operations are supported, by which providers and models, and why a route was selected or rejected.
- As a developer, I want public protocol handling to be built around typed adapters and a canonical model so I can add or modify behavior without touching unrelated paths.
- As an agent-driven contributor, I want work to be divisible into small issues with stable boundaries, clear fixtures, and predictable tests.

## Success Metrics

- All public inference endpoints are served by one canonical runtime pipeline.
- Route explanation for a request matches actual runtime execution for that same request shape.
- Provider selection rejects unsupported capability combinations before execution.
- Protocol adapters are independently testable using golden fixtures.
- Dashboard clearly exposes protocol matrix, capability matrix, route explanation, and request replay.
- The redesign can be implemented through small issues, each completable in one PR with focused tests.

## Constraints

- Canonical runtime types must be protocol-agnostic and transport-agnostic.
- Provider executors must not depend on public protocol DTOs.
- Route planning must depend on declared capabilities, not implicit translator availability.
- Lossy conversions are not allowed silently. Unsupported requests must fail explicitly with a structured reason.
- Dashboard preview and explain must share the same backend planner and capability logic used by the runtime.

## Open Questions

- [x] Should one public protocol be preferred over the others?  
  **Decision:** No. OpenAI, Claude, and Gemini are all first-class ingress protocols, but they normalize into one canonical model.
- [x] Should protocol-specific endpoints be allowed to bypass the canonical runtime for performance?  
  **Decision:** No. All public inference endpoints must use the same runtime pipeline. Operational consistency is more important than endpoint-local optimizations.
- [x] Should lossy provider fallback be allowed for partially supported features?  
  **Decision:** No. The planner may only choose `native` or `lossless` execution modes. Unsupported capability combinations are rejected before execution.
- [x] Should the dashboard remain YAML-first?  
  **Decision:** No. The control plane should be schema-driven and capability-driven first, with raw config editing treated as an advanced operation.

## Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|--------------------|--------|-----------|
| Runtime abstraction | OpenAI-centric dispatch vs canonical IR | Canonical IR | Removes protocol bias and makes routing capability-aware |
| Public protocol support | OpenAI-only primary vs tri-protocol ingress | Tri-protocol ingress | Matches product intent and removes ambiguity |
| Routing model | Model-only routing vs capability-aware routing | Capability-aware routing | Prevents invalid protocol/provider combinations |
| Endpoint architecture | Per-endpoint custom handlers vs unified runtime | Unified runtime | Makes explain, preview, retry, and execution consistent |
| Dashboard focus | Config editor first vs protocol/capability first | Protocol/capability first | Better matches operator mental model |
| Implementation workflow | Large branch refactor vs issue-driven vertical slices | Issue-driven vertical slices | Easier for Claude Code to modify safely and review incrementally |
