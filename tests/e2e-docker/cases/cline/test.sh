#!/usr/bin/env bash
# @level: full
set -euo pipefail

source /tests/lib/helpers.sh

CASE_NAME="cline"
MODEL="qwen3-coder-plus"

log_info "Cline E2E test — OpenAI-compatible mode"

# 1. Verify model is available
check_model_available "$MODEL"

# 2. Install Cline CLI
log_info "Installing Cline CLI..."
npm install -g cline 2>&1 | tail -1
log_info "Cline installed"

# 3. Authenticate with OpenAI-compatible provider (non-interactive)
log_info "Configuring Cline auth..."
cline auth \
    -p openai-compatible \
    -k "sk-proxy-e2e-dummy" \
    -b "http://gateway:8317/v1" \
    -m "$MODEL" 2>&1 || {
    log_warn "cline auth returned non-zero, continuing anyway"
}
log_info "Cline auth configured"

# 4. Set up git workspace
WORKDIR=$(mktemp -d)
cd "$WORKDIR"
git init -q
git config user.email "e2e@test.local"
git config user.name "E2E Test"
git commit --allow-empty -m "init" -q

# 5. Run non-interactive
log_info "Testing model: $MODEL"
timer_start

OUTPUT=$(cline -m "$MODEL" -y "Respond with exactly: PONG" 2>&1) || {
    rc=$?
    elapsed=$(timer_elapsed)
    log_fail "$MODEL — cline exited with code $rc ($(format_duration "$elapsed"))"
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

log_header "Cline test passed"
