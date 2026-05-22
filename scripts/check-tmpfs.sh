#!/bin/bash
# check-tmpfs.sh — Constitution V (SSD Preservation) guard.
#
# Errors loudly when `target/` is not a tmpfs-backed symlink. Used as a
# prerequisite by `make build`, `make test`, `make bench`, etc.
#
# Exit codes:
#   0 — target/ is correctly a symlink to /tmp/cargonaut/<hash>/target,
#       OR the guard is intentionally bypassed (CI=true / waiver env var),
#       OR target/ does not exist yet (first-build case is fine; the next
#       `make tmpfs-setup` invocation will create it).
#   1 — target/ is a real directory on the host SSD with no waiver.
#
# Bypasses (in order of precedence):
#   CI=true                        → CI runners are ephemeral; no SSD to spare
#   CARGONAUT_ALLOW_SSD_TARGET=1   → per-session waiver (see Constitution §V)
#
# See .specify/memory/constitution.md §V for the rule + waiver protocol.
set -euo pipefail

# CI runners: ephemeral containers, no persistent SSD. Exempt automatically.
if [ "${CI:-}" = "true" ]; then
    exit 0
fi

# Per-session waiver. The constitution requires this to be recorded in
# Learnings.md or the PR body when used; we print a reminder.
if [ "${CARGONAUT_ALLOW_SSD_TARGET:-}" = "1" ]; then
    echo "[check-tmpfs] WAIVED via CARGONAUT_ALLOW_SSD_TARGET=1." >&2
    echo "[check-tmpfs] Record the reason in Learnings.md per Constitution §V." >&2
    exit 0
fi

# target/ doesn't exist yet: first-build case. `make tmpfs-setup` should be
# run before the first build, but a missing dir is recoverable, not a
# violation in itself — the *next* build is what would materialize it on
# disk. Warn but don't error.
if [ ! -e target ] && [ ! -L target ]; then
    echo "[check-tmpfs] target/ does not exist yet." >&2
    echo "[check-tmpfs] Run \`make tmpfs-setup\` before \`cargo build\` to redirect" >&2
    echo "[check-tmpfs] build artifacts into tmpfs. See Constitution §V." >&2
    exit 0
fi

# Happy path: target/ is a symlink. Verify it points into /tmp.
if [ -L target ]; then
    link_target="$(readlink -f target 2>/dev/null || true)"
    case "$link_target" in
        /tmp/*)
            # Good — symlink into tmpfs.
            exit 0
            ;;
        "")
            echo "[check-tmpfs] ERROR: target/ is a dangling symlink." >&2
            echo "[check-tmpfs] Run \`make tmpfs-setup\` to repair." >&2
            exit 1
            ;;
        *)
            echo "[check-tmpfs] ERROR: target/ is a symlink to $link_target," >&2
            echo "[check-tmpfs] which is NOT under /tmp. Constitution §V requires" >&2
            echo "[check-tmpfs] tmpfs backing. Remove and re-run \`make tmpfs-setup\`." >&2
            exit 1
            ;;
    esac
fi

# Violation: target/ is a real directory on the host SSD.
echo "[check-tmpfs] ERROR: target/ is a real directory on the host SSD." >&2
echo "[check-tmpfs] Constitution §V (SSD Preservation, NON-NEGOTIABLE) requires" >&2
echo "[check-tmpfs] that target/ be a tmpfs symlink on this dev host." >&2
echo "" >&2
echo "[check-tmpfs] Fix:" >&2
echo "[check-tmpfs]   make tmpfs-setup       # migrates existing artifacts into tmpfs" >&2
echo "" >&2
echo "[check-tmpfs] Waive (only with documented justification — see Constitution §V):" >&2
echo "[check-tmpfs]   CARGONAUT_ALLOW_SSD_TARGET=1 make build" >&2
exit 1
