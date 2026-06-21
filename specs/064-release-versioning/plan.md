# Implementation Plan: Release Process & Version Management (issue #95)

**Branch**: `064-release-versioning` | **Date**: 2026-06-21 | **Spec**: [spec.md](./spec.md)

## Summary

Docs + one CI workflow + a preflight make target + a CHANGELOG restructure. No
crate code changes; the shipped binary is unaffected.

- `docs/VERSIONING.md` — SemVer + 0.y.z rules, single version source, MSRV,
  breaking-change definition (FR-001).
- `docs/RELEASING.md` — the step-by-step checklist (FR-002), referencing
  `make release-check` and the tag-triggered workflow.
- `CHANGELOG.md` — add a Keep-a-Changelog header + an `## [Unreleased]` collector
  above the existing flat per-feature log (kept verbatim under a
  `## [0.1.0] — historical` heading so nothing is lost) (FR-003).
- `.github/workflows/release.yml` — on `v*` tags: checkout, build `make dist`
  (static musl), extract the CHANGELOG section for the tag (fail if missing,
  FR-006), `sha256sum` the tarball, `gh release create` with artifact+checksum+
  notes (FR-004).
- `scripts/release/release-check.sh` + `make release-check` — assert: clean tree,
  tag/version match (when run for a tag), CHANGELOG has the version section
  (FR-005). Wired into `.PHONY`/header/help (FR-007).

## Research decisions

- **R1 — tag-triggered release workflow.** A `push: tags: ['v*']` workflow is the
  standard, idempotent trigger; the tag is the single source of the release
  version and notes are derived from the CHANGELOG section matching it.
- **R2 — reuse `make dist`.** The musl static tarball target already exists and is
  the artifact; the workflow just runs it on an `ubuntu-latest` with the musl
  target installed, then attaches `dist/*.tar.gz` + a `.sha256`.
- **R3 — CHANGELOG: preserve history, add structure.** Rather than rewrite 30+
  flat entries into per-version sections (lossy, error-prone), wrap the existing
  log under a single historical 0.1.0 heading and start the Keep-a-Changelog
  discipline (`Unreleased` → new version sections) going forward.
- **R4 — fail-closed notes.** If no CHANGELOG section matches the tag, the
  workflow exits non-zero before creating the release (FR-006) — no empty/auto
  release notes.

## Constitution Check

- **I. Code Quality** — PASS: docs + shell + YAML; shellcheck-clean script.
- **II. Test-First** — N/A to docs; `make release-check` is the executable gate
  (SC-002), exercised locally. Release publish (SC-003) verified on first tag.
- **III/IV/V** — PASS: no UI/keymap; no perf path; `make dist` already tmpfs-safe;
  release runs on ephemeral CI.

**SC → gate**: SC-001/SC-002 `make release-check` (run locally); SC-003/SC-004 the
release workflow (verified on the first real tag).

## Project Structure

```text
docs/VERSIONING.md            # NEW — versioning policy
docs/RELEASING.md             # NEW — release checklist
CHANGELOG.md                  # restructured: Unreleased + [0.1.0] historical wrapper
.github/workflows/release.yml # NEW — tag-triggered build + GitHub Release
scripts/release/release-check.sh  # NEW — preflight
Makefile                      # release-check target (+ .PHONY/header/help)
README.md                     # link to RELEASING/VERSIONING
```

**Structure Decision**: keep all release machinery in docs + CI + one script;
zero changes to shipped crates.

## Complexity Tracking
> No violations.
