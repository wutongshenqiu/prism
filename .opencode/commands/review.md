---
description: "Review a pull request against project conventions"
---
RUN gh pr diff $1
RUN gh pr view $1 --json title,body,headRefName,baseRefName,files

Review the diff above against project conventions (see AGENTS.md):

- **Code style**: Rust Edition 2024 conventions, proper error handling (thiserror), async-trait usage
- **Type safety**: proper Rust types, no unwrap() in production code, proper error propagation
- **API conventions**: consistent endpoint patterns, proper error response format (ProxyError)
- **Test coverage**: new code should have corresponding tests
- **Security**: no hardcoded API keys, no secrets in code, proper auth checks
- **Provider compatibility**: changes should not break existing provider support
- **Spec & Reference doc sync**: do docs/reference/ updates needed? Is there a related Spec?
- **Config compatibility**: config.yaml changes should be backward compatible

Output a structured review:

```
## PR Review: #$1 — <title>

### Summary
<brief description>

### Findings

#### Critical
<issues that must be fixed before merge>

#### Warning
<issues that should be addressed>

#### Info
<suggestions and observations>

### Doc Impact
<docs that need updating>

### Verdict
<APPROVE / REQUEST_CHANGES / COMMENT>
```
