Review a pull request. Argument $ARGUMENTS: PR number.

Steps:
1. Get PR info: `gh pr view $ARGUMENTS --json title,body,headRefName,baseRefName,files`
2. Get diff (safe mode, no checkout): `gh pr diff $ARGUMENTS`
3. Review the diff against project conventions (CLAUDE.md + AGENTS.md):
   - **Code style**: Rust Edition 2024 conventions, proper error handling (thiserror), async-trait usage
   - **Type safety**: proper Rust types, no unwrap() in production code, proper error propagation
   - **API conventions**: consistent endpoint patterns, proper error response format (ProxyError)
   - **Test coverage**: new code should have corresponding tests
   - **Security**: no hardcoded API keys, no secrets in code, proper auth checks
   - **Provider compatibility**: changes should not break existing provider support
   - **Spec & Reference doc sync**: do docs/reference/ updates needed? Is there a related Spec?
   - **Config compatibility**: config.yaml changes should be backward compatible
   - **Dashboard/control-plane truth**（如相关）:
     - config write/reload 路径是否仍然分叉
     - UI badge / page state 是否来自 backend/runtime truth，而不是前端猜测
     - websocket / request logs 是否考虑了 filters、page 语义和可见连接状态
     - 浏览器测试是否覆盖真实页面契约，而不只是 mocked smoke
4. Output a structured review:

```
## PR Review: #{PR_NUMBER} — {title}

### Summary
<brief description of what the PR does>

### Findings

#### 🔴 Critical
<issues that must be fixed before merge>

#### 🟡 Warning
<issues that should be addressed>

#### 🔵 Info
<suggestions and observations>

### Doc Impact
<list any docs/reference/ or docs/specs/ files that need updating>

### Verdict
<APPROVE / REQUEST_CHANGES / COMMENT>
```

对 dashboard/control-plane PR，优先找这四类问题：
- 假阳性的绿色状态
- 重复的配置写路径
- 实时链路的隐藏失效模式
- 缺少 live browser contract coverage
