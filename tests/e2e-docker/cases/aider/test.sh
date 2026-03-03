#!/usr/bin/env bash
# @level: full
set -euo pipefail

source /tests/lib/helpers.sh

CASE_NAME="aider"
MODEL="qwen3-coder-plus"

log_info "Aider E2E test — OpenAI-compatible mode"

# 1. Verify model is available
check_model_available "$MODEL"

# 2. Install Python + Aider (node:20-bookworm-slim has apt)
log_info "Installing Python and Aider..."
apt-get install -y --no-install-recommends -qq python3 python3-pip python3-venv > /dev/null 2>&1
python3 -m pip install --break-system-packages -q aider-chat 2>&1 | tail -1
log_info "Aider installed"

# 3. Configure: Aider uses env vars for OpenAI-compatible
export OPENAI_API_KEY="sk-proxy-e2e-dummy"
export OPENAI_API_BASE="http://gateway:8317/v1"

# 4. Set up git workspace
WORKDIR=$(mktemp -d)
cd "$WORKDIR"
git init -q
git config user.email "e2e@test.local"
git config user.name "E2E Test"
git commit --allow-empty -m "init" -q

# 5. Run non-interactive with --message flag
log_info "Testing model: $MODEL"
timer_start

OUTPUT=$(aider --model "openai/$MODEL" --no-auto-commits --message "Respond with exactly: PONG" 2>&1) || {
    rc=$?
    elapsed=$(timer_elapsed)
    log_fail "$MODEL — aider exited with code $rc ($(format_duration "$elapsed"))"
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

log_header "Aider test passed"
