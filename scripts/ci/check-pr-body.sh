#!/usr/bin/env bash
# scripts/ci/check-pr-body.sh
#
# PR-body gate for feature-branch PRs.
# Per the project's root-cause-citation discipline, every feature PR body must name the
# concrete root cause. This script enforces the lightweight structural
# version of that rule: the body must contain a `## Summary` or
# `## Root cause` heading, and any `## Root cause` heading must be
# accompanied by at least one `file.ext:line` reference somewhere in
# the body.
#
# Pass (exit 0):
#   - Branch is not a feature branch (does not match ^[0-9]{3}-)
#   - OR any commit message merge-base..HEAD contains [no-docs] (case-insensitive)
#   - OR no PR is currently open for this branch (local-only invocation;
#     the same checks run on PR open via the GitHub Actions workflow)
#   - OR the PR body contains a `## Summary` heading
#   - OR the PR body contains a `## Root cause` heading AND at least one
#     `file.ext:line` reference
# Fail (exit 1):
#   - Feature branch + no [no-docs] + PR exists + body missing required heading
#   - OR body has `## Root cause` heading but no `file.ext:line` reference
# Internal error (exit 2):
#   - Cannot determine merge base / cannot reach gh

set -u

BASE_REF="${BASE_REF:-main}"

# Resolve branch name (mirrors docs-gate.sh).
BRANCH="${GITHUB_HEAD_REF:-}"
if [ -z "${BRANCH}" ]; then
    BRANCH="${GITHUB_REF_NAME:-}"
fi
if [ -z "${BRANCH}" ]; then
    BRANCH="$(git rev-parse --abbrev-ref HEAD 2>/dev/null || true)"
fi
if [ -z "${BRANCH}" ]; then
    printf 'check-pr-body: ERROR — could not determine branch name\n' >&2
    exit 2
fi

# Skip non-feature branches.
if ! printf '%s' "${BRANCH}" | grep -qE '^[0-9]{3}-'; then
    printf "check-pr-body: skipping (branch '%s' is not a feature branch)\n" "${BRANCH}"
    exit 0
fi

# Find merge base; same lookup pattern as docs-gate.
MERGE_BASE=""
if git rev-parse --verify -q "refs/remotes/origin/${BASE_REF}" >/dev/null; then
    MERGE_BASE="$(git merge-base "origin/${BASE_REF}" HEAD 2>/dev/null || true)"
fi
if [ -z "${MERGE_BASE}" ] && git rev-parse --verify -q "refs/heads/${BASE_REF}" >/dev/null; then
    MERGE_BASE="$(git merge-base "${BASE_REF}" HEAD 2>/dev/null || true)"
fi
if [ -z "${MERGE_BASE}" ]; then
    printf 'check-pr-body: ERROR — could not compute merge-base with %s\n' "${BASE_REF}" >&2
    exit 2
fi

# Bypass for [no-docs] PRs (same token as docs-gate so the two stay in sync).
if git log --format=%B "${MERGE_BASE}..HEAD" | grep -iqF '[no-docs]'; then
    printf 'check-pr-body: bypassed via [no-docs] token in commit message\n'
    exit 0
fi

# Resolve the PR body. Two paths:
#   (a) GitHub Actions: gh pr view <PR_NUMBER> --json body works when
#       GH_TOKEN or GITHUB_TOKEN is set in the env.
#   (b) Local invocation: gh pr view --json body picks the PR for the
#       current branch.
# If gh is not installed OR no PR is open for the branch, skip with an
# informational message — local runs before opening a PR pass this gate;
# the same gate runs at PR-open time in CI to enforce the rule.

if ! command -v gh >/dev/null 2>&1; then
    printf 'check-pr-body: skipping (gh CLI not available — install gh or run in CI)\n'
    exit 0
fi

PR_NUMBER="${GITHUB_PR_NUMBER:-}"

# Capture gh exit code separately so we can distinguish:
#   (a) no PR open → skip   (gh exits non-zero)
#   (b) PR open, empty body → FAIL (gh exits 0, body is "")
if [ -n "${PR_NUMBER}" ]; then
    BODY="$(gh pr view "${PR_NUMBER}" --json body --jq .body 2>/dev/null)"
    GH_RC=$?
else
    BODY="$(gh pr view --json body --jq .body 2>/dev/null)"
    GH_RC=$?
fi

if [ "${GH_RC}" -ne 0 ]; then
    printf 'check-pr-body: skipping (no open PR for branch %s)\n' "${BRANCH}"
    exit 0
fi

# Look for ## Summary OR ## Root cause as a level-2 heading (case-insensitive,
# tolerant of trailing whitespace / colon).
HAS_SUMMARY="no"
HAS_ROOTCAUSE="no"
if printf '%s\n' "${BODY}" | grep -qiE '^##[[:space:]]+summary[[:space:]:]*$'; then
    HAS_SUMMARY="yes"
fi
if printf '%s\n' "${BODY}" | grep -qiE '^##[[:space:]]+root[[:space:]]?cause[[:space:]:]*$'; then
    HAS_ROOTCAUSE="yes"
fi

if [ "${HAS_SUMMARY}" = "no" ] && [ "${HAS_ROOTCAUSE}" = "no" ]; then
    cat >&2 <<'EOF'
check-pr-body: FAIL — feature PR body missing required heading.

Per the project's root-cause-citation discipline, every feature PR body must contain at least one of:

  ## Summary
  ## Root cause

The Summary form is for new-feature work. The Root cause form is for
bug-fix work — and additionally requires at least one file.ext:line
reference (e.g. crates/cli/src/main.rs:151) somewhere in the body so a
future reader can find the actual code change without re-reading the
diff.

To fix:
  - Add a `## Summary` (or `## Root cause`) section near the top of
    the PR body. Edit via the GitHub UI or `gh pr edit`.

To bypass (docs-only / infra-only PRs):
  - Include the token [no-docs] in any commit message in this branch.
EOF
    exit 1
fi

# If Root cause heading present, require a file:line reference.
if [ "${HAS_ROOTCAUSE}" = "yes" ]; then
    if ! printf '%s\n' "${BODY}" | grep -qE '[a-zA-Z0-9_/.-]+\.(rs|c|cc|cpp|h|hpp|sh|py|toml|yml|yaml|md|MD):[0-9]+'; then
        cat >&2 <<'EOF'
check-pr-body: FAIL — `## Root cause` PR body missing file.ext:line reference.

When the PR body uses the `## Root cause` heading, the body must also
contain at least one file:line reference so a future reader can jump
directly to the code being fixed.

Example:  in `crates/cli/src/main.rs:151`, `t.pairs[i] = PairEntry::new()`
constructed a 131 KiB struct on the kernel stack, overflowing into
adjacent kstack frames.

To fix:
  - Add at least one file.ext:line citation to the PR body, ideally in
    the section that explains what went wrong.
EOF
        exit 1
    fi
fi

if [ "${HAS_ROOTCAUSE}" = "yes" ] && [ "${HAS_SUMMARY}" = "yes" ]; then
    printf 'check-pr-body: passed (both Summary and Root cause headings present)\n'
elif [ "${HAS_ROOTCAUSE}" = "yes" ]; then
    printf 'check-pr-body: passed (Root cause heading + file:line reference present)\n'
else
    printf 'check-pr-body: passed (Summary heading present)\n'
fi
exit 0
