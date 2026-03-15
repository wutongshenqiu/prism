---
name: review
description: "Review a pull request against project conventions. Provide the PR number."
---

# PR Reviewer

Review a pull request. The user provides a PR number.

Steps:
1. Get PR info: `gh pr view <PR_NUMBER> --json title,body,headRefName,baseRefName,files`
2. Get diff (safe mode, no checkout): `gh pr diff <PR_NUMBER>`
3. Review the diff against project conventions (see AGENTS.md):
   - **Code style**: Rust Edition 2024 conventions, proper error handling (thiserror), async-trait usage
   - **Type safety**: proper Rust types, no unwrap() in production code, proper error propagation
   - **API conventions**: consistent endpoint patterns, proper error response format (ProxyError)
   - **Test coverage**: new code should have corresponding tests
   - **Security**: no hardcoded API keys, no secrets in code, proper auth checks
   - **Provider compatibility**: changes should not break existing provider support
   - **Spec & Reference doc sync**: do docs/reference/ updates needed? Is there a related Spec?
   - **Config compatibility**: config.yaml changes should be backward compatible
   - **Dashboard/control-plane truth** when relevant:
     - are config writes centralized or duplicated?
     - does UI derive badges/state from backend/runtime truth, or from frontend assumptions?
     - do websocket/live-log flows respect filters, page semantics, and visible connection state?
     - is browser coverage live and contract-relevant, or only mocked smoke coverage?
4. Output a structured review:

```
## PR Review: #<PR_NUMBER> — <title>

### Summary
<brief description of what the PR does>

### Findings

#### Critical
<issues that must be fixed before merge>

#### Warning
<issues that should be addressed>

#### Info
<suggestions and observations>

### Doc Impact
<list any docs/reference/ or docs/specs/ files that need updating>

### Verdict
<APPROVE / REQUEST_CHANGES / COMMENT>
```

For dashboard/control-plane PRs, findings should bias toward:
- misleading green states
- duplicated write/reload paths
- hidden realtime failure modes
- tests that miss real page contracts
