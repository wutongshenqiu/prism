# SPEC-032: Frontend Testing Infrastructure

## Status: Active

## Overview

Set up comprehensive testing infrastructure for the React/TypeScript dashboard frontend using Vitest, @testing-library/react, and MSW.

## Goals

1. Establish Vitest test runner with jsdom environment
2. Test all Zustand stores (authStore, metricsStore, logsStore)
3. Test API service layer (interceptors, type mappings)
4. Test reusable components (StatusBadge, MetricCard, ProtectedRoute)
5. Integrate frontend tests into CI pipeline

## Deliverables

- `web/vitest.config.ts` — Vitest configuration
- `web/src/__tests__/setup.ts` — Test setup (jest-dom, localStorage mock)
- 3 store test suites (22 tests)
- 1 API service test suite (8 tests)
- 3 component test suites (18 tests)
- CI workflow job for frontend checks
- Makefile `web-test` target

## Test Count: 48
