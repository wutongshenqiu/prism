# Spec Registry

All specifications for the AI Proxy Gateway project.

## Completed

| ID       | Title                                          | Status    | Location                        |
|----------|------------------------------------------------|-----------|---------------------------------|
| SPEC-001 | Multi-Provider Routing & Credential Management | Completed | [completed/SPEC-001/](completed/SPEC-001/) |
| SPEC-002 | Cross-Format Translation                       | Completed | [completed/SPEC-002/](completed/SPEC-002/) |
| SPEC-003 | SSE Streaming                                  | Completed | [completed/SPEC-003/](completed/SPEC-003/) |
| SPEC-004 | Configuration System & Hot-Reload              | Completed | [completed/SPEC-004/](completed/SPEC-004/) |
| SPEC-005 | Request Retry & Resilience                     | Completed | [completed/SPEC-005/](completed/SPEC-005/) |
| SPEC-006 | Security & Authentication                      | Completed | [completed/SPEC-006/](completed/SPEC-006/) |
| SPEC-007 | Request Cloaking & Payload Rules               | Completed | [completed/SPEC-007/](completed/SPEC-007/) |
| SPEC-008 | 支持 Daemon                                    | Completed | [completed/SPEC-008/](completed/SPEC-008/) |
| SPEC-012 | Rate Limiting                                  | Completed | [completed/SPEC-012/](completed/SPEC-012/) |
| SPEC-013 | Model Fallback & Debug Mode                    | Completed | [completed/SPEC-013/](completed/SPEC-013/) |
| SPEC-014 | Cost Tracking                                  | Completed | [completed/SPEC-014/](completed/SPEC-014/) |
| SPEC-030 | Provider & Dispatch Unit Tests                 | Completed | [completed/SPEC-030/](completed/SPEC-030/) |
| SPEC-036 | Add RequestContext to ProviderExecutor          | Deprecated | [completed/SPEC-036/](completed/SPEC-036/) — *No concrete use case; context already available in dispatch layer* |
| SPEC-037 | Split dispatch.rs into Focused Modules         | Completed | [completed/SPEC-037/](completed/SPEC-037/) |
| SPEC-038 | Unify Provider Request Building                | Completed | [completed/SPEC-038/](completed/SPEC-038/) |
| SPEC-039 | Routing & Auth Key Enhancement                 | Completed | [completed/SPEC-039/](completed/SPEC-039/) |
| SPEC-040 | Request Log & Full-Chain Tracing              | Completed | [completed/SPEC-040/](completed/SPEC-040/) |
| SPEC-041 | LogStore Abstraction & Dashboard UI Overhaul  | Completed | [completed/SPEC-041/](completed/SPEC-041/) |
| SPEC-043 | Dashboard Config Workspace                     | Completed | [completed/SPEC-043/](completed/SPEC-043/) |
| SPEC-044 | Coding Agent Compatibility Endpoints            | Completed | [completed/SPEC-044/](completed/SPEC-044/) |
| SPEC-045 | Model Rewrite Rules                              | Completed | [completed/SPEC-045/](completed/SPEC-045/) |
| SPEC-046 | Dashboard Auth & Security Hardening               | Completed | [completed/SPEC-046/](completed/SPEC-046/) |
| SPEC-047 | OAuth & Auth-File Upstream Onboarding           | Deprecated | [completed/SPEC-047/](completed/SPEC-047/) — *Superseded by SPEC-057* |
| SPEC-048 | Routing Config & Core Types                      | Completed | [completed/SPEC-048/](completed/SPEC-048/) |
| SPEC-049 | Route Planner & Match Engine                     | Completed | [completed/SPEC-049/](completed/SPEC-049/) |
| SPEC-050 | Health State & Selection Strategies              | Completed | [completed/SPEC-050/](completed/SPEC-050/) |
| SPEC-051 | Execution Controller & Dispatch Cutover          | Completed | [completed/SPEC-051/](completed/SPEC-051/) |
| SPEC-052 | Preview API & Dashboard UX                       | Completed | [completed/SPEC-052/](completed/SPEC-052/) |
| SPEC-053 | Thinking Signature Cache                       | Completed | [completed/SPEC-053/](completed/SPEC-053/) |
| SPEC-054 | Extended Thinking Cross-Provider Translation   | Completed | [completed/SPEC-054/](completed/SPEC-054/) |
| SPEC-055 | Gemini Native API Endpoints                    | Completed | [completed/SPEC-055/](completed/SPEC-055/) |
| SPEC-056 | Gemini Multimodal Enhancement                  | Completed | [completed/SPEC-056/](completed/SPEC-056/) |
| SPEC-057 | OAuth & Auth-File Provider Authentication      | Completed | [completed/SPEC-057/](completed/SPEC-057/) |
| SPEC-058 | Provider-Scoped Routing & Amp Integration      | Completed | [completed/SPEC-058/](completed/SPEC-058/) |
| SPEC-059 | Structured Output Translation                  | Completed | [completed/SPEC-059/](completed/SPEC-059/) |
| SPEC-060 | Reverse Translation Paths                      | Completed | [completed/SPEC-060/](completed/SPEC-060/) |
| SPEC-061 | Quota-Aware Credential Switching               | Completed | [completed/SPEC-061/](completed/SPEC-061/) |
| SPEC-062 | Vertex AI Provider                             | Completed | [completed/SPEC-062/](completed/SPEC-062/) |
| SPEC-063 | Unified Provider Configuration                 | Completed | [completed/SPEC-063/](completed/SPEC-063/) |
| SPEC-064 | Upstream Presentation Layer                    | Completed | [completed/SPEC-064/](completed/SPEC-064/) |
| SPEC-065 | Canonical Multi-Protocol Gateway & Control Plane Redesign | Completed | [completed/SPEC-065/](completed/SPEC-065/) |

## Active

| ID       | Title                                          | Status    | Location                        |
|----------|------------------------------------------------|-----------|---------------------------------|
| SPEC-042 | Crate Structure Refactoring                    | Draft     | [active/SPEC-042/](active/SPEC-042/) |

## Retroactively Completed

These specs were implemented before formal tracking was in place.

| ID       | Title                                          | Status    | Location                        |
|----------|------------------------------------------------|-----------|---------------------------------|
| SPEC-009 | Dashboard Admin API & WebSocket                | Completed | [completed/SPEC-009/](completed/SPEC-009/) |
| SPEC-010 | Web Dashboard - Monitoring                     | Completed | [completed/SPEC-010/](completed/SPEC-010/) |
| SPEC-011 | Web Dashboard - Configuration & Operations     | Completed | [completed/SPEC-011/](completed/SPEC-011/) |
| SPEC-029 | Translator Unit Tests                          | Completed | [completed/SPEC-029/](completed/SPEC-029/) |
| SPEC-032 | Frontend Testing Infrastructure                | Completed | [completed/SPEC-032/](completed/SPEC-032/) |
| SPEC-034 | Translator & Server Refactoring                | Completed | [completed/SPEC-034/](completed/SPEC-034/) |
| SPEC-035 | Frontend Code Cleanup                          | Completed | [completed/SPEC-035/](completed/SPEC-035/) |

## How to Create a New Spec

1. Copy the appropriate template from `_templates/`
2. Assign the next SPEC-NNN ID
3. Place in `active/SPEC-NNN/`
4. Add an entry to this registry table under **Active**
5. When complete, move to `completed/SPEC-NNN/` and update status here

See [playbooks/create-new-spec.md](../playbooks/create-new-spec.md) for detailed instructions.
