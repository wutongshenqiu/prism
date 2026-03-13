# Technical Design: Dashboard Config Workspace

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-043       |
| Title     | Dashboard Config Workspace |
| Author    | Claude          |
| Status    | Active         |
| Created   | 2026-03-13     |
| Updated   | 2026-03-13     |

## Overview

Add a Config page to the dashboard with read/edit/validate/reload workflow. Requires one new backend endpoint and a new frontend page.

## API Design

### New Endpoint

```
GET /api/dashboard/config/raw
```

Response:
```json
{
  "content": "host: 0.0.0.0\nport: 8317\n...",
  "path": "/path/to/config.yaml"
}
```

### Modified Endpoint

```
POST /api/dashboard/config/validate
```

Now accepts either JSON config or YAML string:
```json
{"yaml": "host: 0.0.0.0\nport: 8317\n..."}
```

## Backend Implementation

- Add `get_raw_config()` handler in `config_ops.rs`
- Enhance `validate_config()` to accept `{"yaml": "..."}` format
- Register new route in dashboard router

## Frontend Implementation

- New `web/src/pages/Config.tsx` page
- Add `/config` route in `App.tsx`
- Add navigation entry in `Layout.tsx`
- Two-tab layout: "Current Config" (read-only) and "Editor" (editable YAML)

## Task Breakdown

- [x] Add GET /api/dashboard/config/raw endpoint
- [x] Enhance validate endpoint for YAML input
- [x] Create Config.tsx page
- [x] Add route and navigation
- [x] Update tests
