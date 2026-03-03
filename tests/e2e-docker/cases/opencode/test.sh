#!/usr/bin/env bash
set -euo pipefail

source /tests/lib/helpers.sh

CASE_NAME="opencode"

log_info "opencode E2E test — all Bailian models"

MODELS=(
    "qwen3.5-plus"
    "qwen3-coder-next"
    "qwen3-coder-plus"
    "glm-5"
    "glm-4.7"
    "kimi-k2.5"
    "MiniMax-M2.5"
)

# 1. Verify all models are available
for model in "${MODELS[@]}"; do
    check_model_available "$model"
done

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
# Removes: apiKey, api-key, authorization headers, tokens
sanitize_json() {
    sed -E \
        -e 's/"(apiKey|api-key|api_key|token|authorization|secret)"\s*:\s*"[^"]*"/"\1": "***"/gi' \
        -e 's/sk-[a-zA-Z0-9_-]+/sk-***/g'
}

# 6. Test each model
MODEL_PASSED=0
MODEL_FAILED=0

for model in "${MODELS[@]}"; do
    log_info "Testing model: $model"
    timer_start

    # Run once with --format json to capture structured event stream
    # This includes: text events, tool_use, reasoning, step_start/finish, errors
    # We extract text content from JSON events for display + include full JSON for details
    RAW_OUTPUT=$(opencode run --format json -m "proxy/$model" "Respond with exactly: PONG" 2>&1) || {
        elapsed=$(timer_elapsed)
        log_fail "$model — opencode exited with code $? ($(format_duration "$elapsed"))"
        echo "$RAW_OUTPUT"

        SANITIZED=$(echo "$RAW_OUTPUT" | sanitize_json)
        report_row "$CASE_NAME" "fail" "$model" "$elapsed" "$SANITIZED"
        ((MODEL_FAILED++)) || true
        continue
    }

    elapsed=$(timer_elapsed)

    if [[ -z "$RAW_OUTPUT" ]]; then
        log_fail "$model — empty output ($(format_duration "$elapsed"))"
        report_row "$CASE_NAME" "fail" "$model" "$elapsed" "(empty output)"
        ((MODEL_FAILED++)) || true
        continue
    fi

    # Extract text content from JSON events for quick summary
    # JSON events use .part.text for text content
    TEXT_CONTENT=$(echo "$RAW_OUTPUT" | jq -r 'select(.type == "text") | .part.text // empty' 2>/dev/null | tr -d '\n' || echo "")
    if [[ -z "$TEXT_CONTENT" ]]; then
        TEXT_CONTENT="(no text extracted from JSON events)"
    fi

    log_info "Response: $TEXT_CONTENT"

    # Sanitize and build report with full JSON event stream
    SANITIZED=$(echo "$RAW_OUTPUT" | sanitize_json)
    COMBINED="[opencode output]
${TEXT_CONTENT}

[Session Messages (sanitized)]
${SANITIZED}"

    log_pass "$model ($(format_duration "$elapsed"))"
    report_row "$CASE_NAME" "pass" "$model" "$elapsed" "$COMBINED"
    ((MODEL_PASSED++)) || true
done

# 7. Cleanup
rm -rf "$WORKDIR"

# 8. Summary
log_header "opencode results: $MODEL_PASSED/$((MODEL_PASSED + MODEL_FAILED)) models passed"

if [[ $MODEL_FAILED -gt 0 ]]; then
    log_fail "$MODEL_FAILED model(s) failed"
    exit 1
fi
