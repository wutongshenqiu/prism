# AI Proxy Gateway — Documentation

## Overview

AI Proxy Gateway is a Rust/Axum multi-provider AI API gateway that routes requests across OpenAI, Claude (Anthropic), and Gemini (Google) APIs. It translates between provider-specific formats, manages credentials and routing, supports SSE streaming, and provides retry/resilience capabilities — all behind a unified API surface that is compatible with standard AI SDKs.

## System Architecture

```
┌──────────┐     ┌──────────────────────────────────────────────┐
│  Client   │────▶│  Server (Axum)                               │
│(OpenAI SDK│     │  ┌─────────┐  ┌──────────┐  ┌───────────┐  │
│ Claude SDK│     │  │  Auth   │─▶│ Dispatch  │─▶│ Translator│  │
│  etc.)    │     │  │Middleware│  │(Route+    │  │(Format    │  │
└──────────┘     │  └─────────┘  │ Retry)    │  │ Convert)  │  │
                  │               └─────┬─────┘  └─────┬─────┘  │
                  │                     │              │         │
                  │               ┌─────▼──────────────▼──────┐  │
                  │               │    Provider Executor       │  │
                  │               │  (Claude/OpenAI/Gemini)    │  │
                  │               └────────────┬──────────────┘  │
                  └────────────────────────────┼────────────────┘
                                               │
                                    ┌──────────▼──────────┐
                                    │   Upstream AI API    │
                                    │(api.anthropic.com,   │
                                    │ api.openai.com, etc.)│
                                    └─────────────────────┘
```

**Request flow:** A client sends a request using any supported SDK format. The Axum server receives it, authenticates via middleware, dispatches to the appropriate provider route (with retry logic), translates the request/response format as needed, and forwards to the upstream AI API.

## SDD (Spec-Driven Development)

This project follows Spec-Driven Development: every significant feature or change begins with a specification document (PRD + Technical Design) before implementation starts. Specs live in `docs/specs/` and progress through `active/` to `completed/` as work is finished. This ensures design decisions are captured, reviewed, and traceable.

## Document Structure

```
docs/
├── README.md                    # This file
├── specs/
│   ├── _index.md               # Spec registry
│   ├── _templates/             # Spec templates
│   │   ├── prd.md
│   │   ├── technical-design.md
│   │   └── research.md
│   ├── active/                 # In-progress specs
│   └── completed/              # Completed specs
│       ├── SPEC-001/
│       ├── ...
│       └── SPEC-007/
├── reference/
│   ├── types/
│   │   ├── enums.md
│   │   ├── config.md
│   │   ├── provider.md
│   │   └── errors.md
│   ├── api-surface.md
│   └── architecture.md
└── playbooks/
    ├── create-new-spec.md
    ├── add-provider.md
    ├── add-translator.md
    └── coding-agent-workflow.md
```

## Reference Documentation

- [Type Definitions — Enums](reference/types/enums.md)
- [Type Definitions — Config](reference/types/config.md)
- [Type Definitions — Provider](reference/types/provider.md)
- [Type Definitions — Errors](reference/types/errors.md)
- [API Surface](reference/api-surface.md)
- [Architecture](reference/architecture.md)

## Spec Index

| ID       | Title                                       | Status    |
|----------|---------------------------------------------|-----------|
| SPEC-001 | Multi-Provider Routing & Credential Management | Completed |
| SPEC-002 | Cross-Format Translation                    | Completed |
| SPEC-003 | SSE Streaming                               | Completed |
| SPEC-004 | Configuration System & Hot-Reload           | Completed |
| SPEC-005 | Request Retry & Resilience                  | Completed |
| SPEC-006 | Security & Authentication                   | Completed |
| SPEC-007 | Request Cloaking & Payload Rules            | Completed |

See [specs/_index.md](specs/_index.md) for the full registry with links and details.

## Playbooks

- [Create New Spec](playbooks/create-new-spec.md)
- [Add Provider](playbooks/add-provider.md)
- [Add Translator](playbooks/add-translator.md)
- [Coding Agent Workflow](playbooks/coding-agent-workflow.md)
- [Deploy To The SSH Host](playbooks/deploy-ssh-host.md)
