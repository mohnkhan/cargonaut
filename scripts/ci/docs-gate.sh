#!/usr/bin/env bash
# scripts/ci/docs-gate.sh
#
# Documentation gate for feature-branch PRs.
# Per Feature 035 contracts/docs-gate.contract.md.
#
# Pass (exit 0):
#   - Branch is not a feature branch (does not match ^[0-9]{3}-)
#   - OR any commit message merge-base..HEAD contains [no-docs] (case-insensitive substring)
#   - OR both Learnings.md AND README.md are modified merge-base..HEAD
# Fail (exit 1):
#   - Feature branch + no [no-docs] token + Learnings.md or README.md missing from changed-files
# Internal error (exit 2):
#   - Cannot determine merge base (origin/master not fetched, no git repo, etc.)

set -u

BASE_REF="${BASE_REF:-main}"

# Resolve branch name from CI env first, then git.
BRANCH="${GITHUB_HEAD_REF:-}"
if [ -z "${BRANCH}" ]; then
    BRANCH="${GITHUB_REF_NAME:-}"
fi
if [ -z "${BRANCH}" ]; then
    BRANCH="$(git rev-parse --abbrev-ref HEAD 2>/dev/null || true)"
fi
if [ -z "${BRANCH}" ]; then
    printf 'docs-gate: ERROR — could not determine branch name\n' >&2
    exit 2
fi

# Skip non-feature branches (anything that doesn't start with NNN-).
if ! printf '%s' "${BRANCH}" | grep -qE '^[0-9]{3}-'; then
    printf "docs-gate: skipping (branch '%s' is not a feature branch)\n" "${BRANCH}"
    exit 0
fi

# Find the merge base. origin/<base> first; fall back to local <base>.
MERGE_BASE=""
if git rev-parse --verify -q "refs/remotes/origin/${BASE_REF}" >/dev/null; then
    MERGE_BASE="$(git merge-base "origin/${BASE_REF}" HEAD 2>/dev/null || true)"
fi
if [ -z "${MERGE_BASE}" ] && git rev-parse --verify -q "refs/heads/${BASE_REF}" >/dev/null; then
    MERGE_BASE="$(git merge-base "${BASE_REF}" HEAD 2>/dev/null || true)"
fi
if [ -z "${MERGE_BASE}" ]; then
    printf 'docs-gate: ERROR — could not compute merge-base with %s (was the branch fetched with full history?)\n' "${BASE_REF}" >&2
    exit 2
fi

# Bypass: case-insensitive substring [no-docs] in any commit msg merge-base..HEAD.
if git log --format=%B "${MERGE_BASE}..HEAD" | grep -iqF '[no-docs]'; then
    printf 'docs-gate: bypassed via [no-docs] token in commit message\n'
    exit 0
fi

# Compute changed files.
CHANGED="$(git diff --name-only "${MERGE_BASE}..HEAD" 2>/dev/null || true)"

LEARNINGS_OK="no"
README_OK="no"
if printf '%s\n' "${CHANGED}" | grep -qx 'Learnings.md'; then LEARNINGS_OK="yes"; fi
if printf '%s\n' "${CHANGED}" | grep -qx 'README.md'; then README_OK="yes"; fi

if [ "${LEARNINGS_OK}" = "yes" ] && [ "${README_OK}" = "yes" ]; then
    printf 'docs-gate: passed (Learnings.md and README.md both updated)\n'
    exit 0
fi

# Failure: at least one missing.
MISSING=""
if [ "${LEARNINGS_OK}" = "no" ]; then MISSING="Learnings.md"; fi
if [ "${README_OK}" = "no" ]; then
    if [ -n "${MISSING}" ]; then MISSING="${MISSING}, README.md"; else MISSING="README.md"; fi
fi

MERGE_BASE_SHORT="$(printf '%s' "${MERGE_BASE}" | cut -c1-7)"

cat <<EOF
docs-gate: FAIL — feature branches must update both Learnings.md and README.md per CLAUDE.md / CONTRIBUTING.md.

Branch:        ${BRANCH}
Merge base:    ${MERGE_BASE_SHORT}
Missing:       ${MISSING}

To fix:
  - Add a Learnings.md entry describing what was hard, root causes, and decisions.
  - Update README.md's "Feature History" or "At a Glance" section to mention the new feature.
  - Push the changes; CI will re-evaluate.

To bypass (docs-only or infra-only PR):
  - Include the token [no-docs] in any commit message in this branch (case-insensitive).
  - Example: git commit --amend -m "Original message [no-docs]"
EOF
exit 1
