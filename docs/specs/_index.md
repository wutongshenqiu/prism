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

## Active

| ID       | Title                                          | Status    | Location                        |
|----------|------------------------------------------------|-----------|---------------------------------|
| SPEC-009 | Dashboard Admin API & WebSocket                | Draft     | [active/SPEC-009/](active/SPEC-009/) |
| SPEC-010 | Web Dashboard - Monitoring                     | Draft     | [active/SPEC-010/](active/SPEC-010/) |
| SPEC-011 | Web Dashboard - Configuration & Operations     | Draft     | [active/SPEC-011/](active/SPEC-011/) |
| SPEC-029 | Translator Unit Tests                          | Active    | [active/SPEC-029/](active/SPEC-029/) |
| SPEC-030 | Provider & Dispatch Unit Tests                 | Active    | [active/SPEC-030/](active/SPEC-030/) |
| SPEC-032 | Frontend Testing Infrastructure                | Active    | [active/SPEC-032/](active/SPEC-032/) |
| SPEC-034 | Translator & Server Refactoring                | Active    | [active/SPEC-034/](active/SPEC-034/) |
| SPEC-035 | Frontend Code Cleanup                          | Active    | [active/SPEC-035/](active/SPEC-035/) |
| SPEC-036 | Add RequestContext to ProviderExecutor          | Draft     | [active/SPEC-036/](active/SPEC-036/) |

## How to Create a New Spec

1. Copy the appropriate template from `_templates/`
2. Assign the next SPEC-NNN ID
3. Place in `active/SPEC-NNN/`
4. Add an entry to this registry table under **Active**
5. When complete, move to `completed/SPEC-NNN/` and update status here

See [playbooks/create-new-spec.md](../playbooks/create-new-spec.md) for detailed instructions.
