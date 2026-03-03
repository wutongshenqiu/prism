#!/usr/bin/env bash
# @level: full
set -euo pipefail

source /tests/lib/helpers.sh

CASE_NAME="claude-code"
MODEL="qwen3-coder-plus"

log_info "Claude Code E2E test — Anthropic protocol via gateway"

# 1. Install Claude Code + jq
log_info "Installing Claude Code..."
npm install -g @anthropic-ai/claude-code 2>&1 | tail -1
apt-get install -y --no-install-recommends -qq jq > /dev/null 2>&1
log_info "Claude Code installed: $(claude --version 2>/dev/null || echo 'unknown')"

# 2. Configure: Claude Code uses Anthropic env vars
export ANTHROPIC_API_KEY="sk-proxy-e2e-dummy"
export ANTHROPIC_BASE_URL="http://gateway:8317"
export DISABLE_AUTOUPDATER=1
export DISABLE_TELEMETRY=1
export CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1

# 3. Sanity check: verify gateway Anthropic endpoint with direct curl
log_info "Verifying Anthropic protocol via curl..."
CURL_RESP=$(curl -sf "http://gateway:8317/v1/messages" \
    -H "Content-Type: application/json" \
    -H "x-api-key: sk-proxy-e2e-dummy" \
    -H "anthropic-version: 2023-06-01" \
    -d "{
        \"model\": \"$MODEL\",
        \"max_tokens\": 64,
        \"messages\": [{\"role\": \"user\", \"content\": \"Respond with exactly: PONG\"}]
    }" 2>&1) || {
    log_fail "Anthropic protocol sanity check failed"
    echo "$CURL_RESP"
    report_row "$CASE_NAME" "fail" "curl-sanity" "0" "$CURL_RESP"
    exit 1
}
log_info "Anthropic endpoint OK: $(echo "$CURL_RESP" | jq -r '.content[0].text // empty' 2>/dev/null | head -c 100)"

# 4. Set up git workspace (Claude Code expects a git repo)
WORKDIR=$(mktemp -d)
cd "$WORKDIR"
git init -q
git config user.email "e2e@test.local"
git config user.name "E2E Test"
git commit --allow-empty -m "init" -q

# 5. Run Claude Code non-interactive (-p = print mode)
log_info "Testing model: $MODEL"
timer_start

OUTPUT=$(claude -p "Respond with exactly: PONG" --model "$MODEL" --max-turns 1 2>&1) || {
    rc=$?
    elapsed=$(timer_elapsed)
    log_fail "$MODEL — claude exited with code $rc ($(format_duration "$elapsed"))"
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

# 6. Cleanup
rm -rf "$WORKDIR"

log_header "Claude Code test passed"
