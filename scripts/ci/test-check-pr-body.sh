#!/usr/bin/env bash
# Self-test for scripts/ci/check-pr-body.sh.
#
# Runs a battery of in-process test cases against the gate script using
# a temp-dir fake `gh` that emits a configurable PR body. Each case
# asserts the expected exit code.
#
# Invoke directly: bash scripts/ci/test-check-pr-body.sh
# Exit code: 0 = all pass; non-zero = at least one failure.

set -u
SCRIPT="$(cd "$(dirname "$0")" && pwd)/check-pr-body.sh"

if [ ! -x "${SCRIPT}" ]; then
    printf 'test-check-pr-body: ERROR — %s missing or not executable\n' "${SCRIPT}" >&2
    exit 2
fi

PASS=0; FAIL=0
TMP="$(mktemp -d)"; trap 'rm -rf "${TMP}"' EXIT

# Fake gh that emits whatever's in $TMP/body.txt and respects $TMP/exit.
cat > "${TMP}/gh" <<'FAKE'
#!/usr/bin/env bash
cat "${TMP_BODY_FILE:-/dev/null}"
exit "${TMP_EXIT_CODE:-0}"
FAKE
chmod +x "${TMP}/gh"

run() {
    local name="$1" expect="$2" body="$3" gh_rc="${4:-0}"
    local body_file="${TMP}/body-${name}.txt"
    printf '%s' "${body}" > "${body_file}"
    local out rc
    out="$(
        PATH="${TMP}:${PATH}" \
        GITHUB_PR_NUMBER=999 \
        TMP_BODY_FILE="${body_file}" \
        TMP_EXIT_CODE="${gh_rc}" \
        bash "${SCRIPT}" 2>&1
    )"
    rc=$?
    if [ "${rc}" = "${expect}" ]; then
        printf 'PASS [%s] rc=%d\n' "${name}" "${rc}"
        PASS=$((PASS + 1))
    else
        printf 'FAIL [%s] expected rc=%s got %d output:\n%s\n' "${name}" "${expect}" "${rc}" "${out}"
        FAIL=$((FAIL + 1))
    fi
}

# Body with Summary heading should pass
run "summary-only" 0 "## Summary
Some text"

# Body with Root cause + file:line should pass
run "rootcause-with-fileref" 0 "## Root cause
The bug is in \`kernel/src/foo.rs:42\` where x was wrong."

# Body with Root cause but NO file:line should fail
run "rootcause-no-fileref" 1 "## Root cause
The bug was hard to find."

# Empty body (PR exists) should fail
run "empty-body" 1 ""

# Body with random heading should fail
run "no-required-heading" 1 "## Notes
This is a PR."

# Case-insensitive headings
run "lower-case-summary" 0 "## summary
ok"

# Both Summary and Root cause + fileref
run "both-headings" 0 "## Summary

## Root cause
fix in foo.rs:1"

# Root cause with file:line embedded in arbitrary context
run "fileref-anywhere" 0 "## Root cause

The issue traces to the implementation at userland/sshd/main.c:284
where the wrong buffer is freed."

# gh non-zero exit (no PR open) → skip
run "no-pr-open" 0 "" 1

printf '\nResults: %d pass, %d fail\n' "${PASS}" "${FAIL}"
exit "${FAIL}"
