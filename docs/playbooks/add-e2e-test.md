# How to Add a New E2E Docker Test

## Overview

The Docker E2E framework (`tests/e2e-docker/`) auto-discovers test cases from `cases/<name>/test.sh`. Adding a new CLI tool test requires **zero changes** to entrypoint, docker-compose, or CI.

## Steps

### 1. Create test directory

```bash
mkdir tests/e2e-docker/cases/<tool-name>/
```

### 2. Create `test.sh`

```bash
#!/usr/bin/env bash
# @level: full          ← quick = every push, full = manual/schedule only
set -euo pipefail

source /tests/lib/helpers.sh

CASE_NAME="<tool-name>"
MODEL="qwen3-coder-plus"

log_info "<Tool> E2E test"

# 1. Verify model
check_model_available "$MODEL"

# 2. Install tool
log_info "Installing <tool>..."
# npm install -g <package> 2>&1 | tail -1
# or: apt-get install ... && pip install ...

# 3. Configure authentication (see Auth Patterns below)
# ...

# 4. Set up git workspace (most coding agents require this)
WORKDIR=$(mktemp -d) && cd "$WORKDIR"
git init -q && git config user.email "e2e@test.local" && git config user.name "E2E"
git commit --allow-empty -m "init" -q

# 5. Run non-interactive
log_info "Testing model: $MODEL"
timer_start

OUTPUT=$(<tool-command> 2>&1) || {
    rc=$?
    elapsed=$(timer_elapsed)
    log_fail "$MODEL — <tool> exited with code $rc ($(format_duration "$elapsed"))"
    echo "$OUTPUT"
    report_row "$CASE_NAME" "fail" "$MODEL" "$elapsed" "$OUTPUT"
    rm -rf "$WORKDIR"
    exit 1
}

elapsed=$(timer_elapsed)

if [[ -z "$OUTPUT" ]]; then
    log_fail "$MODEL — empty output ($(format_duration "$elapsed"))"
    report_row "$CASE_NAME" "fail" "$MODEL" "$elapsed" "(empty output)"
    rm -rf "$WORKDIR"
    exit 1
fi

log_info "Response: $OUTPUT"
log_pass "$MODEL ($(format_duration "$elapsed"))"
report_row "$CASE_NAME" "pass" "$MODEL" "$elapsed" "$OUTPUT"

rm -rf "$WORKDIR"
log_header "<Tool> test passed"
```

### 3. Choose level

- `# @level: quick` — lightweight smoke test, runs on every push to main (~15s budget)
- `# @level: full` — comprehensive test, runs on manual dispatch and weekly schedule

### 4. Test locally

```bash
# Quick tests only (default)
make test-e2e-docker

# Full suite
TEST_LEVEL=full make test-e2e-docker

# Single test
TEST_FILTER=<tool-name> TEST_LEVEL=full make test-e2e-docker
```

## Auth Patterns by Protocol

### OpenAI-compatible tools

Tools that support `OPENAI_API_KEY` / `OPENAI_BASE_URL` env vars:

```bash
export OPENAI_API_KEY="sk-proxy-e2e-dummy"
export OPENAI_BASE_URL="http://gateway:8317/v1"
```

**Aider** uses a different env var name:
```bash
export OPENAI_API_KEY="sk-proxy-e2e-dummy"
export OPENAI_API_BASE="http://gateway:8317/v1"
```

**Cline** requires explicit auth command (env vars don't work):
```bash
cline auth -p openai -k "sk-proxy-e2e-dummy" -b "http://gateway:8317/v1" -m "$MODEL"
```

### Anthropic-protocol tools

Tools that use the Anthropic Messages API (`/v1/messages`):

```bash
export ANTHROPIC_API_KEY="sk-proxy-e2e-dummy"
export ANTHROPIC_BASE_URL="http://gateway:8317"
```

Requires `claude-api-key` section in `config.e2e.yaml`.

## Gateway Config

Test cases use models configured in `tests/e2e-docker/config.e2e.yaml`:

- `openai-compatibility` section — OpenAI protocol (`/v1/chat/completions`)
- `claude-api-key` section — Anthropic protocol (`/v1/messages`)

If your tool requires a new model or provider, add it to this config file.

## Helper Functions

Available in `tests/e2e-docker/lib/helpers.sh`:

| Function | Usage |
|----------|-------|
| `check_model_available MODEL` | Verify model exists via `/v1/models` |
| `timer_start` / `timer_elapsed` | Millisecond precision timing |
| `format_duration MS` | Human-readable duration string |
| `report_row CASE STATUS NAME DURATION OUTPUT` | Write test result to report |
| `log_info/log_pass/log_fail/log_warn/log_header` | Colored log output |

## Known Gotchas

- `((var++))` with `set -e` fails when var=0 — always add `|| true`
- Container base image is `node:20-bookworm-slim` — has apt but no python by default
- Gateway hostname in docker-compose network is `gateway`, not `localhost`
- Capture `$?` immediately in error handlers — `timer_elapsed` overwrites it
