---
description: "Tasks — Feature 064 release process & version management (issue #95)"
---

# Tasks: Release Process & Version Management

## Phase 1 — US2: Versioning policy (P2)
- [ ] T001 [US2] Write `docs/VERSIONING.md`: SemVer + 0.y.z rules, single source `[workspace.package] version`, MSRV policy, breaking-change definition (FR-001).

## Phase 2 — US1: Release process + automation (P1)
- [ ] T002 [US1] Restructure `CHANGELOG.md`: add Keep-a-Changelog header + `## [Unreleased]` collector; wrap the existing flat log under `## [0.1.0] — historical` (no history lost) (FR-003).
- [ ] T003 [US1] Write `scripts/release/release-check.sh`: assert clean tree, `[workspace.package] version` parseable, a CHANGELOG section exists for that version (and, when `$GITHUB_REF`/arg is a tag, tag==version); shellcheck-clean (FR-005).
- [ ] T004 [US1] `make release-check` → runs the script; update `.PHONY`, header comment, `help` (FR-005/FR-007).
- [ ] T005 [US1] Write `.github/workflows/release.yml`: trigger on `push: tags: ['v*']`; install musl target; `make dist`; extract the CHANGELOG section for the tag (fail if missing, FR-006); `sha256sum` the tarball; `gh release create <tag> dist/*.tar.gz dist/*.sha256 --notes-file <section>` (FR-004).
- [ ] T006 [US1] Write `docs/RELEASING.md`: the step-by-step checklist (bump → CHANGELOG → `make release-check` → `make ci-local` → annotated tag → push → automation) (FR-002).

## Phase 3 — Polish
- [ ] T007 [P] README: link VERSIONING.md + RELEASING.md; verify `make help | grep release-check`.
- [ ] T008 [P] Verify: `make release-check` passes on a prepared commit and fails on a forced version/CHANGELOG mismatch (SC-002); `yaml`-lint the workflow; docs (Learnings ≥3 bullets, README metrics if any, CHANGELOG Unreleased entry); ROADMAP #95 resolved.
- [ ] T009 [P] Final gate: `make ci-local`; clippy/fmt clean (no crate changes, but run anyway).

## MVP
US1 (RELEASING.md + release-check + workflow) — the repeatable release path.

## Note
Actually cutting/pushing `v0.1.0` is deferred to explicit go-ahead (outward-facing).
EOF
