#!/usr/bin/env bash
# @level: quick
set -euo pipefail

source /tests/lib/helpers.sh

CASE_NAME="opencode"

log_info "opencode E2E smoke test — single model (quick)"

MODEL="qwen3-coder-plus"

# 1. Verify model is available
check_model_available "$MODEL"

# 2. Install opencode + jq for JSON processing
log_info "Installing opencode and jq..."
npm install -g opencode-ai 2>&1 | tail -1
apt-get install -y --no-install-recommends -qq jq > /dev/null 2>&1
log_info "opencode installed"

# 3. Set up workspace (opencode requires a git repo)
WORKDIR=$(mktemp -d)
cd "$WORKDIR"
git init -q
git config user.email "e2e@test.local"
git config user.name "E2E Test"
git commit --allow-empty -m "init" -q

# 4. Configure opencode
cp /tests/cases/opencode/opencode.json "$WORKDIR/opencode.json"

# 5. Sanitize sensitive data from session JSON
sanitize_json() {
    sed -E \
        -e 's/"(apiKey|api-key|api_key|token|authorization|secret)"\s*:\s*"[^"]*"/"\1": "***"/gi' \
        -e 's/sk-[a-zA-Z0-9_-]+/sk-***/g'
}

# 6. Test single model
log_info "Testing model: $MODEL"
timer_start

RAW_OUTPUT=$(opencode run --format json -m "proxy/$MODEL" "Respond with exactly: PONG" 2>&1) || {
    elapsed=$(timer_elapsed)
    log_fail "$MODEL — opencode exited with code $? ($(format_duration "$elapsed"))"
    echo "$RAW_OUTPUT"

    SANITIZED=$(echo "$RAW_OUTPUT" | sanitize_json)
    report_row "$CASE_NAME" "fail" "$MODEL" "$elapsed" "$SANITIZED"
    rm -rf "$WORKDIR"
    exit 1
}

elapsed=$(timer_elapsed)

if [[ -z "$RAW_OUTPUT" ]]; then
    log_fail "$MODEL — empty output ($(format_duration "$elapsed"))"
    report_row "$CASE_NAME" "fail" "$MODEL" "$elapsed" "(empty output)"
    rm -rf "$WORKDIR"
    exit 1
fi

# Extract text content from JSON events
TEXT_CONTENT=$(echo "$RAW_OUTPUT" | grep '^{' | jq -r 'select(.type == "text") | .part.text // empty' 2>/dev/null | tr -d '\n' || echo "")
if [[ -z "$TEXT_CONTENT" ]]; then
    TEXT_CONTENT="(no text extracted from JSON events)"
fi

log_info "Response: $TEXT_CONTENT"

SANITIZED=$(echo "$RAW_OUTPUT" | sanitize_json)
COMBINED="[opencode output]
${TEXT_CONTENT}

[Session Messages (sanitized)]
${SANITIZED}"

log_pass "$MODEL ($(format_duration "$elapsed"))"
report_row "$CASE_NAME" "pass" "$MODEL" "$elapsed" "$COMBINED"

# 7. Cleanup
rm -rf "$WORKDIR"

log_header "opencode smoke test passed"
