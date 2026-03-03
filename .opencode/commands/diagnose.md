---
description: "Diagnose and fix project issues"
---

Diagnose and fix the following issue: $1

Steps:
1. Analyze the problem and locate likely modules:
   - Routing/credential → crates/provider/src/routing.rs, crates/core/src/config.rs
   - Translation → crates/translator/src/
   - SSE/streaming → crates/provider/src/sse.rs, crates/server/src/streaming.rs
   - Config/hot-reload → crates/core/src/config.rs
   - Auth → crates/server/src/auth.rs
   - Cloaking/Payload → crates/core/src/cloak.rs, crates/core/src/payload.rs
   - Dispatch/retry → crates/server/src/dispatch.rs
   - Provider execution → crates/provider/src/
2. Examine related code paths, error handling (ProxyError), config.example.yaml
3. Attempt to reproduce: check existing tests, construct minimal test case if needed
4. Locate root cause and implement fix
5. Run `make test` to verify no regressions
6. Run `make lint` to ensure code style
7. Summarize: root cause → fix → verification → doc update needed?
