#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
FIXTURE_PATH="${PRISM_E2E_FIXTURE:-$ROOT_DIR/web/e2e/fixtures/prism.playwright.yaml}"
RUN_DIR="$(mktemp -d "${TMPDIR:-/tmp}/prism-playwright.XXXXXX")"
CONFIG_PATH="$RUN_DIR/config.yaml"

cleanup() {
  rm -rf "$RUN_DIR"
}

trap cleanup EXIT

cp "$FIXTURE_PATH" "$CONFIG_PATH"

cd "$ROOT_DIR"
exec cargo run --quiet -- run --config "$CONFIG_PATH" --log-level warn
