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
| SPEC-037 | Split dispatch.rs into Focused Modules         | Completed | [completed/SPEC-037/](completed/SPEC-037/) |
| SPEC-038 | Unify Provider Request Building                | Completed | [completed/SPEC-038/](completed/SPEC-038/) |
| SPEC-039 | Routing & Auth Key Enhancement                 | Completed | [completed/SPEC-039/](completed/SPEC-039/) |
| SPEC-040 | Request Log & Full-Chain Tracing              | Completed | [completed/SPEC-040/](completed/SPEC-040/) |
| SPEC-041 | LogStore Abstraction & Dashboard UI Overhaul  | Completed | [completed/SPEC-041/](completed/SPEC-041/) |
| SPEC-043 | Dashboard Config Workspace                     | Completed | [completed/SPEC-043/](completed/SPEC-043/) |
| SPEC-044 | Coding Agent Compatibility Endpoints            | Completed | [completed/SPEC-044/](completed/SPEC-044/) |
| SPEC-045 | Model Rewrite Rules                              | Completed | [completed/SPEC-045/](completed/SPEC-045/) |
| SPEC-046 | Dashboard Auth & Security Hardening               | Completed | [completed/SPEC-046/](completed/SPEC-046/) |
| SPEC-048 | Routing Config & Core Types                      | Completed | [completed/SPEC-048/](completed/SPEC-048/) |
| SPEC-049 | Route Planner & Match Engine                     | Completed | [completed/SPEC-049/](completed/SPEC-049/) |
| SPEC-050 | Health State & Selection Strategies              | Completed | [completed/SPEC-050/](completed/SPEC-050/) |
| SPEC-051 | Execution Controller & Dispatch Cutover          | Completed | [completed/SPEC-051/](completed/SPEC-051/) |
| SPEC-052 | Preview API & Dashboard UX                       | Completed | [completed/SPEC-052/](completed/SPEC-052/) |

## Active

| ID       | Title                                          | Status    | Location                        |
|----------|------------------------------------------------|-----------|---------------------------------|
| SPEC-030 | Provider & Dispatch Unit Tests                 | Active    | [active/SPEC-030/](active/SPEC-030/) |
| SPEC-036 | Add RequestContext to ProviderExecutor          | Draft     | [active/SPEC-036/](active/SPEC-036/) |
| SPEC-042 | Crate Structure Refactoring                    | Draft     | [active/SPEC-042/](active/SPEC-042/) |
| SPEC-047 | OAuth & Auth-File Upstream Onboarding             | Draft     | [active/SPEC-047/](active/SPEC-047/) |

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
