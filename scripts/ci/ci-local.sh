#!/usr/bin/env bash
# scripts/ci/ci-local.sh
#
# Local CI driver: runs the same step sequence as .github/workflows/ci.yml
# so a contributor can verify green-or-fail before pushing. Exits 0 on green;
# non-zero on the first failed step. Failure artifacts land under
# dist/ci-artifacts/.
#
# Adapted from the MyOS2026 Feature 035 pattern.

set -u

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "${REPO_ROOT}" || exit 2

ARTIFACT_DIR="${REPO_ROOT}/dist/ci-artifacts"
STDOUT_LOG="/tmp/ci-test-stdout.log"
STDERR_LOG="/tmp/ci-test-stderr.log"

# Steps that are allowed to fail without blocking the local run.
# Mirrors the FLAKY allowlist in .github/workflows/ci.yml's `ci` rollup
# job. Keep both in sync.
declare -A FLAKY=(
    # [step-name]="#NN reason"
)

FLAKED=()

banner() { printf '\n--- ci-local: step %s ---\n' "$1"; }
header() { printf '=== ci-local: %s ===\n' "$1"; }

header "starting"
rm -rf "${ARTIFACT_DIR}"
rm -f "${STDOUT_LOG}" "${STDERR_LOG}"

# Run a single step; on failure, either record as flaked (if allow-listed
# in FLAKY) and continue, or bundle artifacts and exit.
# Args: <step-label> <timeout-spec> <bundle-job-name> <bundle-axis> -- <cmd...>
run_step() {
    local label="$1" tspec="$2" bundle_job="$3" bundle_axis="$4"
    shift 4
    [ "$1" = "--" ] && shift
    banner "${label}"
    if timeout "${tspec}" "$@" > "${STDOUT_LOG}" 2> "${STDERR_LOG}"; then
        local rc=0
    else
        local rc=$?
    fi
    cat "${STDOUT_LOG}"
    [ -s "${STDERR_LOG}" ] && cat "${STDERR_LOG}" >&2
    if [ "${rc}" -ne 0 ]; then
        CI_FAILED_STEP="${label}" \
            bash "${REPO_ROOT}/scripts/ci/bundle-failure-artifacts.sh" "${bundle_job}" "${bundle_axis}" || true
        if [ -n "${FLAKY[$label]+x}" ]; then
            FLAKED+=("${label} (exit ${rc}, allowed-flaky: ${FLAKY[$label]})")
            header "FLAKED ${label} (exit ${rc}; allowed by FLAKY allowlist — ${FLAKY[$label]}); continuing"
            return 0
        fi
        header "FAILED at step ${label} (exit ${rc}); artifacts in ${ARTIFACT_DIR}"
        exit "${rc}"
    fi
}

# 1. fmt
run_step "fmt"       "2m"  "fmt"       "none" -- cargo fmt --all -- --check

# 2. clippy
run_step "clippy"    "10m" "clippy"    "none" -- cargo clippy --workspace --all-targets -- -D warnings

# 3. unit tests
run_step "unit-test" "15m" "unit-test" "none" -- cargo test --workspace --all-targets

# 4. release build (compile-time gate; catches release-only issues)
run_step "build"     "15m" "build"     "none" -- cargo build --release --workspace

# 5. docs-gate (only meaningful on feature branches; passes otherwise)
run_step "docs-gate" "1m"  "docs-gate" "none" -- bash "${REPO_ROOT}/scripts/ci/docs-gate.sh"

# 6. check-pr-body (only fires if a PR is open for this branch)
run_step "check-pr-body" "1m" "check-pr-body" "none" -- bash "${REPO_ROOT}/scripts/ci/check-pr-body.sh"

# Final report
echo
header "DONE"
if [ "${#FLAKED[@]}" -gt 0 ]; then
    printf 'Allowed-flaky steps (passed locally with non-zero exit):\n'
    for f in "${FLAKED[@]}"; do printf '  - %s\n' "$f"; done
fi
echo "ALL GREEN"
exit 0
