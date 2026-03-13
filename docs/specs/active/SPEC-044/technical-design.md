# Technical Design: Coding Agent Compatibility Endpoints

| Field     | Value          |
|-----------|----------------|
| Spec ID   | SPEC-044       |
| Title     | Coding Agent Compatibility Endpoints |
| Author    | Claude          |
| Status    | Active         |
| Created   | 2026-03-13     |
| Updated   | 2026-03-13     |

## Overview

Add two new API endpoints for better coding agent compatibility.

## API Design

### POST /v1/completions
Routes to same dispatch logic as chat completions (Format::OpenAI).

### POST /v1/messages/count_tokens
Proxies to Claude's count_tokens endpoint via the existing credential routing.

## Task Breakdown

- [x] Add completions handler
- [x] Add count_tokens handler
- [x] Register routes
- [x] Tests
