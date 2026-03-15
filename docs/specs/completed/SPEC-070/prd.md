# PRD: Codex Device Flow, Local Import, and Responses WebSocket

| Field     | Value |
|-----------|-------|
| Spec ID   | SPEC-070 |
| Title     | Codex Device Flow, Local Import, and Responses WebSocket |
| Status    | Completed |
| Created   | 2026-03-15 |
| Updated   | 2026-03-15 |

## Summary

Prism must support Codex managed auth without depending on browser automation, and it must expose a native websocket entrypoint for the OpenAI Responses protocol that works with real Codex sessions.

## Requirements

1. Dashboard must support three Codex connection paths:
   - Browser OAuth
   - Device flow
   - Server-local `~/.codex/auth.json` import
2. Managed Codex tokens must remain runtime-only and never be written back to `config.yaml`.
3. Dashboard session auth must not accept `?token=` query parameters.
4. Public API must expose `/v1/responses/ws` and provider-scoped `/api/provider/{provider}/v1/responses/ws`.
5. The websocket path must pin the successful credential and preserve Codex `previous_response_id` semantics across turns.

## Validation

- Rust integration tests for dashboard auth/device/import flows
- Web tests for dashboard connect UX
- Playwright coverage for Codex OAuth connect flow after the UI changes
- Live verification against a real Codex account
