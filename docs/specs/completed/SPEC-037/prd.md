# SPEC-037: Split dispatch.rs into Focused Modules

## Problem

`dispatch.rs` is 1017 lines with multiple responsibilities: retry logic, payload rules, cloaking, cost tracking, model fallback, streaming, and keepalive. This creates high cognitive load for maintainers.

## Requirements

1. Split `dispatch.rs` into smaller, focused modules under `dispatch/` directory
2. Zero behavior change — pure refactor
3. All existing tests pass without modification
4. Each extracted module has clear, single responsibility

## Non-Goals

- Adding new features or changing dispatch behavior
- Adding new tests (though extracted modules become easier to test)

## Success Criteria

- `dispatch.rs` reduced to orchestrator (~300 lines)
- Helper logic extracted to focused modules
- All tests pass with zero regression
