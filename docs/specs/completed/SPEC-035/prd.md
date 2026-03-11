# SPEC-035: Frontend Code Cleanup

## Problem

1. **Type duplication**: `MetricsState`, `AuthState`, `LogsState` interfaces are defined in both store files and `types/index.ts`.
2. **Utility duplication**: `formatUptime()` has two different implementations in `System.tsx` and `Overview.tsx`.

## Goals

- Remove duplicate store-local types from `types/index.ts`
- Unify `formatUptime()` into a single utility function
- Extract `formatNumber()` utility

## Success Criteria

- All 48 frontend tests pass
- No TypeScript errors (`npx tsc --noEmit`)
- No duplicate type definitions
