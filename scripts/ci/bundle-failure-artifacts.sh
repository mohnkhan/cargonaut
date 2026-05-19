#!/usr/bin/env bash
# scripts/ci/bundle-failure-artifacts.sh <job-name> <matrix-axis-or-"none">
#
# Assemble dist/ci-artifacts/ for upload by actions/upload-artifact@v4.
# Adapted from the MyOS2026 Feature 035 pattern, slimmed for a Rust
# userland project (no QEMU serial logs to collect).
#
# Steps:
#   1. mkdir -p dist/ci-artifacts/
#   2. Locate /tmp/ci-test-stdout.log and /tmp/ci-test-stderr.log → test-{stdout,stderr}.log
#   3. Truncate each to a reasonable size (head + tail, 4 KiB total)
#   4. Generate README.txt with run metadata
#
# Inputs (env, all optional with sensible defaults):
#   GITHUB_RUN_ID, GITHUB_SHA, GITHUB_HEAD_REF, GITHUB_REF_NAME, GITHUB_EVENT_NAME
# Inputs (positional):
#   $1 = job name (e.g. "unit-test"); used in README.txt
#   $2 = matrix axis value (or literal "none")

set -u

if [ "$#" -ne 2 ]; then
    printf 'usage: %s <job-name> <matrix-axis-or-none>\n' "$0" >&2
    exit 2
fi

JOB_NAME="$1"
MATRIX_AXIS="$2"

OUT_DIR="dist/ci-artifacts"
mkdir -p "$OUT_DIR"

# Helper: copy a log file with size truncation. Keeps the first 2 KiB and
# last 2 KiB, separating with a "[... truncated ...]" marker. Keeps small
# files verbatim.
copy_truncated() {
    local src="$1"
    local dst="$2"
    if [ ! -f "$src" ]; then
        : > "$dst"
        return
    fi
    local size
    size=$(wc -c < "$src")
    if [ "$size" -le 4096 ]; then
        cp "$src" "$dst"
    else
        {
            head -c 2048 "$src"
            printf '\n\n[... truncated %d bytes ...]\n\n' "$((size - 4096))"
            tail -c 2048 "$src"
        } > "$dst"
    fi
}

copy_truncated /tmp/ci-test-stdout.log "$OUT_DIR/test-stdout.log"
copy_truncated /tmp/ci-test-stderr.log "$OUT_DIR/test-stderr.log"

cat > "$OUT_DIR/README.txt" <<EOF
Cargonaut CI failure bundle

Job:           ${JOB_NAME}
Matrix axis:   ${MATRIX_AXIS}
Run ID:        ${GITHUB_RUN_ID:-local}
Commit SHA:    ${GITHUB_SHA:-unknown}
Branch (HEAD): ${GITHUB_HEAD_REF:-${GITHUB_REF_NAME:-local}}
Event:         ${GITHUB_EVENT_NAME:-push}

Files in this bundle:
  test-stdout.log   Captured stdout from the failing step (truncated head+tail)
  test-stderr.log   Captured stderr from the failing step (truncated head+tail)

Reproduce locally:
  make ci-local
EOF

printf 'Bundle prepared at %s\n' "$OUT_DIR"
ls -la "$OUT_DIR" || true
