#!/usr/bin/env bash
# scripts/check-binary-size.sh
#
# NFR-001 gate: release build must produce a `cargonaut` binary that
# is ≤ 8 MiB after `strip`. Fails (exit 1) if larger so a single
# unintended bloat dep is caught before merge.
#
# Usage:
#   bash scripts/check-binary-size.sh
#
# Env overrides:
#   MAX_SIZE_MIB  — soft override of the cap (default 8). Useful for
#                   bisecting bloat regressions.

set -euo pipefail

MAX_SIZE_MIB="${MAX_SIZE_MIB:-8}"
MAX_SIZE_BYTES=$((MAX_SIZE_MIB * 1024 * 1024))

BIN_PATH="target/release/cargonaut"

if [ ! -f "${BIN_PATH}" ]; then
    printf 'check-binary-size: building release...\n' >&2
    cargo build --release --bin cargonaut
fi

if [ ! -f "${BIN_PATH}" ]; then
    printf 'check-binary-size: ERROR — %s not found after release build\n' "${BIN_PATH}" >&2
    exit 2
fi

# Strip into a temp copy so the working release build isn't mutated
# (handy for `cargo run` after this script).
STRIPPED="$(mktemp)"
trap 'rm -f "${STRIPPED}"' EXIT
cp "${BIN_PATH}" "${STRIPPED}"
if command -v strip >/dev/null 2>&1; then
    strip "${STRIPPED}"
else
    printf 'check-binary-size: WARN — strip(1) not available; measuring un-stripped\n' >&2
fi

ACTUAL_BYTES="$(stat -c '%s' "${STRIPPED}" 2>/dev/null || stat -f '%z' "${STRIPPED}")"
ACTUAL_MIB="$(awk "BEGIN { printf \"%.2f\", ${ACTUAL_BYTES} / 1024 / 1024 }")"

printf 'check-binary-size: cargonaut stripped = %s MiB (cap = %s MiB)\n' \
    "${ACTUAL_MIB}" "${MAX_SIZE_MIB}"

if [ "${ACTUAL_BYTES}" -gt "${MAX_SIZE_BYTES}" ]; then
    printf 'check-binary-size: FAIL — %s MiB > %s MiB ceiling (NFR-001)\n' \
        "${ACTUAL_MIB}" "${MAX_SIZE_MIB}" >&2
    printf '  Diagnose with: cargo bloat --release --bin cargonaut\n' >&2
    exit 1
fi

printf 'check-binary-size: OK\n'
