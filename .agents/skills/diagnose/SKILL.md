---
name: diagnose
description: "Diagnose and fix project issues. Provide a problem description."
---

# Problem Diagnostor

Diagnose and fix project issues. The user provides a problem description.

Steps:
1. Analyze the problem description and locate likely modules:
   - Routing/credential issues → crates/provider/src/routing.rs, crates/core/src/config.rs
   - Translation errors → crates/translator/src/ (corresponding translator module)
   - SSE/streaming issues → crates/provider/src/sse.rs, crates/server/src/streaming.rs
   - Config/hot-reload → crates/core/src/config.rs (ConfigWatcher)
   - Auth issues → crates/server/src/auth.rs
   - Cloaking/Payload → crates/core/src/cloak.rs, crates/core/src/payload.rs
   - Request dispatch/retry → crates/server/src/dispatch/ (mod.rs, helpers.rs, streaming.rs, retry.rs)
   - Provider execution → crates/provider/src/ (corresponding executor)
2. Examine related code paths:
   - Read potentially involved source files
   - Check error handling paths (ProxyError variants)
   - Check config.example.yaml for related config items
3. Attempt to reproduce:
   - Check if related tests cover this scenario
   - If needed, construct a minimal test case
4. Locate root cause and implement fix
5. Run `make test` to verify the fix does not introduce new issues
6. Run `make lint` to ensure code style compliance
7. Summarize: root cause → fix content → verification results → whether docs need updating
