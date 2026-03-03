#!/usr/bin/env bash
set -euo pipefail

source /tests/lib/helpers.sh

log_header "E2E Docker Test Runner"

# 1. Install shared dependencies
log_info "Installing shared dependencies..."
apt-get update -qq && apt-get install -y --no-install-recommends -qq curl git > /dev/null 2>&1
log_info "Dependencies installed"

# 2. Initialize report
report_init

# 3. Wait for gateway
wait_for_health "$GATEWAY_URL/health" 30

# 4. Discover & run test cases
FILTER="${TEST_FILTER:-}"
PASSED=0
FAILED=0
SKIPPED=0

timer_start
SUITE_START=$_TIMER_START

for test_dir in /tests/cases/*/; do
    name=$(basename "$test_dir")

    # Apply filter if set
    if [[ -n "$FILTER" && "$name" != *"$FILTER"* ]]; then
        log_info "Skipping: $name (filtered)"
        ((SKIPPED++)) || true
        continue
    fi

    # Skip directories without test.sh
    if [[ ! -f "$test_dir/test.sh" ]]; then
        log_warn "Skipping: $name (no test.sh)"
        ((SKIPPED++)) || true
        continue
    fi

    log_header "Running: $name"
    timer_start
    if bash "$test_dir/test.sh"; then
        local_elapsed=$(timer_elapsed)
        log_pass "$name ($(format_duration "$local_elapsed"))"
        ((PASSED++)) || true
    else
        local_elapsed=$(timer_elapsed)
        log_fail "$name ($(format_duration "$local_elapsed"))"
        ((FAILED++)) || true
    fi
done

# Calculate total duration
_TIMER_START=$SUITE_START
TOTAL_DURATION=$(timer_elapsed)

# 5. Generate report
report_generate "$PASSED" "$FAILED" "$SKIPPED" "$TOTAL_DURATION"

# Copy report to output volume (if mounted)
if [[ -d /output ]]; then
    cp "$REPORT_DIR/report.md" /output/report.md
    log_info "Report copied to /output/report.md"
fi

# Print report to stdout for local visibility
echo ""
cat "$REPORT_DIR/report.md"

# 6. Summary
log_header "Results: $PASSED passed, $FAILED failed, $SKIPPED skipped ($(format_duration "$TOTAL_DURATION"))"

if [[ $PASSED -eq 0 && $FAILED -eq 0 ]]; then
    log_warn "No tests were executed"
    exit 1
fi

[[ $FAILED -eq 0 ]]
