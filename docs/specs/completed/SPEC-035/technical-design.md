# SPEC-035: Technical Design — Frontend Code Cleanup

## 2a. Remove type duplication

Remove `MetricsState`, `AuthState`, `LogsState` from `web/src/types/index.ts` — stores are the SSOT.

## 2b. Extract shared utility functions

Create `web/src/utils/format.ts` with unified `formatUptime()` and `formatNumber()`.
Update `System.tsx` and `Overview.tsx` to import from utils.
