# PRD: Dashboard Config Workspace

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-043       |
| Title     | Dashboard Config Workspace |
| Author    | Claude          |
| Status    | Active         |
| Created   | 2026-03-13     |
| Updated   | 2026-03-13     |

## Problem Statement

Prism exposes config endpoints but lacks a dashboard page for viewing, editing, validating, and applying configuration changes. Operators must SSH into the server and edit YAML files manually.

## Goals

- Add a dedicated Config page to the dashboard SPA
- Support viewing current sanitized config and raw YAML
- Provide validate-before-apply workflow with clear error feedback
- Support reload action with confirmation and status feedback

## Non-Goals

- Config version history / audit trail
- Visual config builder (form-based editing per section)
- Multi-file config support

## User Stories

- As an operator, I want to view the current config from the dashboard so I can verify settings remotely.
- As an operator, I want to edit YAML config and validate before applying so I avoid breaking changes.
- As an operator, I want to reload config from the dashboard so I don't need SSH access.

## Success Metrics

- Config page accessible via dashboard navigation
- Validate + reload workflow functions end-to-end
- TypeScript compiles with no errors

## Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|--------------------|--------|-----------|
| Editor | Monaco, CodeMirror, textarea | textarea with monospace | No heavy deps, sufficient for YAML editing |
| Config read | New raw endpoint vs enhance current | New GET /config/raw | Keep sanitized endpoint separate from raw |
