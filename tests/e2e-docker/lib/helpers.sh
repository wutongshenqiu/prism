#!/usr/bin/env bash
# Shared test utilities for E2E Docker tests.
# Sourced by entrypoint.sh and inherited by all test cases.

set -euo pipefail

# --- Environment ---
GATEWAY_URL="${GATEWAY_URL:-http://gateway:8317}"
REPORT_DIR="${REPORT_DIR:-/tmp/e2e-report}"

# --- Colors ---
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# --- Logging ---
log_info()   { echo -e "${BLUE}[INFO]${NC}  $*"; }
log_pass()   { echo -e "${GREEN}[PASS]${NC}  $*"; }
log_fail()   { echo -e "${RED}[FAIL]${NC}  $*"; }
log_warn()   { echo -e "${YELLOW}[WARN]${NC}  $*"; }
log_header() { echo -e "\n${BOLD}=== $* ===${NC}"; }

# --- Timer ---
# Uses millisecond precision via date +%s%3N.
# Usage:
#   timer_start
#   ... do work ...
#   elapsed=$(timer_elapsed)
_TIMER_START=0

timer_start() {
    _TIMER_START=$(date +%s%3N)
}

timer_elapsed() {
    local now
    now=$(date +%s%3N)
    echo $(( now - _TIMER_START ))
}

# Format milliseconds to human-readable string
format_duration() {
    local ms="$1"
    if [[ $ms -lt 1000 ]]; then
        echo "${ms}ms"
    elif [[ $ms -lt 60000 ]]; then
        local secs=$((ms / 1000))
        local remainder=$((ms % 1000))
        printf "%d.%03ds" "$secs" "$remainder"
    else
        local mins=$((ms / 60000))
        local secs=$(( (ms % 60000) / 1000 ))
        printf "%dm%ds" "$mins" "$secs"
    fi
}

# --- Report ---
# Each test item saves: metadata (status, duration) + full output to separate files.
# Structure:
#   $REPORT_DIR/<case>/           — per-case directory
#   $REPORT_DIR/<case>/<name>.meta  — "status|duration_ms"
#   $REPORT_DIR/<case>/<name>.out   — full captured output (multiline)
#   $REPORT_DIR/<case>/_order.txt   — insertion order of test names

report_init() {
    mkdir -p "$REPORT_DIR"
}

# Save a single test item result with full output
# Usage: report_row <case> <status> <name> <duration_ms> <output>
report_row() {
    local case="$1"
    local status="$2"
    local name="$3"
    local duration_ms="$4"
    local output="${5:-}"
    local dir="$REPORT_DIR/$case"

    mkdir -p "$dir"
    echo "${status}|${duration_ms}" > "$dir/${name}.meta"
    echo "$output" > "$dir/${name}.out"
    echo "$name" >> "$dir/_order.txt"
}

# Generate final Markdown report with collapsible full output
# Output: writes to $REPORT_DIR/report.md
report_generate() {
    local total_passed="$1"
    local total_failed="$2"
    local total_skipped="$3"
    local total_duration="$4"
    local report="$REPORT_DIR/report.md"

    {
        echo "# E2E Docker Test Report"
        echo ""
        echo "> $(date -u '+%Y-%m-%d %H:%M:%S UTC') | Duration: $(format_duration "$total_duration")"
        echo ""

        # Overall summary
        if [[ $total_failed -eq 0 ]]; then
            echo "**Result: ALL PASSED** ($total_passed passed, $total_skipped skipped)"
        else
            echo "**Result: FAILED** ($total_passed passed, $total_failed failed, $total_skipped skipped)"
        fi
        echo ""

        # Per-case sections
        for case_dir in "$REPORT_DIR"/*/; do
            [[ ! -d "$case_dir" ]] && continue
            [[ ! -f "$case_dir/_order.txt" ]] && continue
            local case_name
            case_name=$(basename "$case_dir")

            echo "## $case_name"
            echo ""

            # Summary table
            echo "| Status | Model | Duration |"
            echo "|--------|-------|----------|"

            local case_pass=0 case_fail=0
            while IFS= read -r name; do
                [[ ! -f "$case_dir/${name}.meta" ]] && continue
                IFS='|' read -r status duration_ms < "$case_dir/${name}.meta"
                local icon duration_str
                case "$status" in
                    pass) icon="✅"; ((case_pass++)) || true ;;
                    fail) icon="❌"; ((case_fail++)) || true ;;
                    skip) icon="⏭️" ;;
                    *)    icon="❓" ;;
                esac
                duration_str=$(format_duration "$duration_ms")
                echo "| $icon | \`$name\` | $duration_str |"
            done < "$case_dir/_order.txt"

            echo ""
            echo "**$case_pass passed, $case_fail failed**"
            echo ""

            # Per-model detail sections with full output
            echo "### Model Output Details"
            echo ""
            while IFS= read -r name; do
                [[ ! -f "$case_dir/${name}.meta" ]] && continue
                IFS='|' read -r status duration_ms < "$case_dir/${name}.meta"
                local icon
                case "$status" in
                    pass) icon="✅" ;; fail) icon="❌" ;; *) icon="⏭️" ;;
                esac

                echo "<details>"
                echo "<summary>$icon <code>$name</code> ($(format_duration "$duration_ms"))</summary>"
                echo ""
                # Render output with smart code block detection:
                # [opencode output] → plain code block
                # [Session Messages (sanitized)] → json code block
                if [[ -f "$case_dir/${name}.out" ]]; then
                    local in_section="" section_type=""
                    while IFS= read -r line; do
                        # Strip ANSI codes
                        line=$(echo "$line" | sed 's/\x1b\[[0-9;]*m//g')
                        case "$line" in
                            "[opencode output]")
                                [[ -n "$in_section" ]] && echo '```' && echo ""
                                echo "**CLI Output**"
                                echo '```'
                                in_section="text"
                                ;;
                            "[Session Messages (sanitized)]")
                                [[ -n "$in_section" ]] && echo '```' && echo ""
                                echo "**Session Messages**"
                                echo '```json'
                                in_section="json"
                                ;;
                            *)
                                echo "$line"
                                ;;
                        esac
                    done < "$case_dir/${name}.out"
                    [[ -n "$in_section" ]] && echo '```'
                fi
                echo ""
                echo "</details>"
                echo ""
            done < "$case_dir/_order.txt"
        done
    } > "$report"

    log_info "Report written to $report"
}

# --- Assertions ---

assert_contains() {
    local haystack="$1"
    local needle="$2"
    if [[ "$haystack" == *"$needle"* ]]; then
        return 0
    else
        log_fail "Expected output to contain '$needle'"
        log_info "Actual output: $haystack"
        return 1
    fi
}

assert_exit_code() {
    local actual="$1"
    local expected="$2"
    if [[ "$actual" -eq "$expected" ]]; then
        return 0
    else
        log_fail "Expected exit code $expected, got $actual"
        return 1
    fi
}

assert_not_empty() {
    local value="$1"
    local label="${2:-value}"
    if [[ -n "$value" ]]; then
        return 0
    else
        log_fail "$label is empty"
        return 1
    fi
}

# --- Infrastructure ---

wait_for_health() {
    local url="${1:-$GATEWAY_URL/health}"
    local max_retries="${2:-30}"
    local retry=0

    log_info "Waiting for $url to be healthy..."
    while [[ $retry -lt $max_retries ]]; do
        if curl -sf "$url" > /dev/null 2>&1; then
            log_info "Service is healthy (after ${retry}s)"
            return 0
        fi
        sleep 1
        ((retry++))
    done

    log_fail "Service not healthy after ${max_retries}s: $url"
    return 1
}

check_model_available() {
    local model="$1"
    local response

    response=$(curl -sf "$GATEWAY_URL/v1/models" 2>&1) || {
        log_fail "Failed to fetch /v1/models"
        return 1
    }

    if echo "$response" | grep -q "\"$model\""; then
        log_info "Model '$model' is available"
        return 0
    else
        log_fail "Model '$model' not found in /v1/models response"
        log_info "Response: $response"
        return 1
    fi
}
