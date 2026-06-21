# Quickstart / Verification Guide: Repository Housekeeping (Feature 058)

How to validate this feature end-to-end. Each check maps to a Success Criterion in
`spec.md`. Run from the repo root on branch `058-repo-housekeeping`.

## Prerequisites

- `target/` symlinked to tmpfs (Constitution §V): `make tmpfs-status` shows the link.
- `gh` authenticated (for the issue-existence check, SC-007).

## SC-001 — requirements.toml reads as historical, no false CI claim

```sh
head -n 15 design/contracts/requirements.toml      # expect a HISTORICAL banner + pointer to specs/NNN/
grep -in "CI greps this file" design/contracts/requirements.toml   # expect: no live assertion (removed or explicitly negated)
```
Expected: banner present in first lines; no surviving sentence claiming CI enforces the file.

## SC-002 — live contracts untouched

```sh
git diff --name-only main...HEAD -- design/contracts/   # expect ONLY requirements.toml
```
Expected: exactly `design/contracts/requirements.toml` (no schema/keymap/commands files).

## SC-003 — Cargo.toml header corrected

```sh
sed -n '1,8p' Cargo.toml
grep -n "Phase 1 in progress" Cargo.toml   # expect: no match
```
Expected: header references the spec-kit workflow (`.specify/feature.json` / `specs/NNN/`);
no "Phase 1 in progress"; no presentation of `design/plan.md` as the live plan.

## SC-004 — orphaned dirs gone

```sh
test ! -e tests/integration && echo "tests/integration removed OK"
test ! -e benches && echo "benches removed OK"
```
Expected: both echo their OK line.

## SC-005 — full pipeline green

```sh
make ci-local        # clippy -D warnings → cargo test --workspace → build --release → check-pr-body → docs-gate
```
Expected: exits 0. (docs-gate passes because README.md + Learnings.md are modified.)

## SC-006 — zero production source changed

```sh
git diff --name-only main...HEAD -- 'crates/*/src/' | sed -n '1,200p'
```
Expected: empty output.

## SC-007 — deferral tracked

```sh
grep -n "cargonaut-core" ROADMAP.md                 # expect one new row referencing an issue #
gh issue list --label follow-up --search "cargonaut-core"   # expect the open issue
```
Expected: one ROADMAP row + one matching open GitHub issue with the `follow-up` label.

## Rollback

All changes are metadata/layout. To revert: `git checkout main -- Cargo.toml
design/contracts/requirements.toml ROADMAP.md README.md Learnings.md` and
`git checkout main -- benches` (restores `.gitkeep`); recreate `tests/integration/` only
if some external tooling expects it (none does — see research R-004). Close the GitHub
issue if the deferral is withdrawn.
