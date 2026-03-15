---
description: "Harden Prism dashboard/control-plane behavior for runtime truth, config safety, realtime logs, and live browser coverage"
---

Use this command when the work is really a dashboard/control-plane product correction, not an isolated bug. Goal: $1

Typical triggers:
- dashboard truth
- config workspace is misleading
- request logs / websocket behavior is wrong
- Models / Protocols / System should reflect runtime truth
- need real Playwright coverage, not mocked-only tests
- several related issues should be solved as one product change

Steps:
1. Regroup the work by product surface instead of inheriting the old issue split:
   - Config mutation truth
   - Runtime truth semantics
   - Realtime logs / websocket productization
   - Browser contract coverage
2. Start from backend truth:
   - `crates/server/src/handler/dashboard/`
   - `crates/server/tests/dashboard_tests.rs`
3. Then inspect the matching frontend surface:
   - `web/src/pages/`
   - `web/src/stores/`
   - `web/src/services/`
   - `web/e2e/`
4. Apply these design rules:
   - keep one shared config write transaction path
   - derive badges/state from backend/runtime truth instead of frontend assumptions
   - make live logs respect filters and page semantics
   - surface websocket connection/reconnect/token-refresh state in the UI
   - delete misleading states instead of preserving guessed semantics
5. Default verification bundle:
   - `make lint`
   - `make test`
   - `cd web && npm run test`
   - `cd web && npm run build`
   - `cd web && npm run test:e2e`
6. Only add remote/live validation when the user explicitly asks:
   - build the current branch
   - start an isolated backend on the remote machine
   - tunnel local Playwright to the remote backend
7. If multiple issues are really one product change:
   - solve the product change first, then map back to issues/specs
   - use closing keywords only when the result fully closes the issue
   - verify issue closure after merge
