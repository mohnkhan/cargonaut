#!/usr/bin/env bash
# Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# Feature 064 release preflight. Verifies the repo is ready to tag/release:
#   1. the workspace version is readable,
#   2. (if a ref/tag is given) the tag matches the workspace version,
#   3. CHANGELOG.md has a section for the version,
#   4. the working tree is clean (unless RELEASE_CHECK_SKIP_TREE=1).
#
# Usage:
#   scripts/release/release-check.sh [vX.Y.Z]
# In CI the tag comes from $GITHUB_REF_NAME; the workflow sets
# RELEASE_CHECK_SKIP_TREE=1 (it builds artifacts into the tree).
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

version="$(sed -n 's/^version *= *"\(.*\)"/\1/p' Cargo.toml | head -1)"
if [ -z "$version" ]; then
  echo "release-check: could not read [workspace.package] version from Cargo.toml" >&2
  exit 1
fi

ref="${1:-${GITHUB_REF_NAME:-}}"
case "$ref" in
  v*)
    tagver="${ref#v}"
    if [ "$tagver" != "$version" ]; then
      echo "release-check: tag '$ref' does not match workspace version '$version'" >&2
      exit 1
    fi
    ;;
esac

# Escape dots so the version matches literally in the heading regex.
esc="${version//./\\.}"
if ! grep -qE "^## \[${esc}\]" CHANGELOG.md; then
  echo "release-check: CHANGELOG.md has no '## [$version]' section" >&2
  echo "  (rename '## [Unreleased]' to '## [$version] — $(date +%F)' before releasing)" >&2
  exit 1
fi

if [ "${RELEASE_CHECK_SKIP_TREE:-0}" != "1" ] && [ -n "$(git status --porcelain)" ]; then
  echo "release-check: working tree is dirty — commit or stash before releasing" >&2
  exit 1
fi

echo "release-check: OK — version $version, CHANGELOG section present${ref:+, ref $ref}"
