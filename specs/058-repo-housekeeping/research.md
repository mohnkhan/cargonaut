# Research: Repository Housekeeping (Feature 058)

Phase 0 output. All claims below were **measured**, not estimated, on branch
`058-repo-housekeeping` at the time of writing.

## R-001 — Is `requirements.toml` actually stale, and by how much?

**Decision**: Treat it as a historical/superseded artifact, not a live manifest.

**Evidence**:
- Extracted every `verification = "<path>"` value and tested existence:
  **59 file-path verifications → 2 resolve, 57 missing (96.6% dead).**
- The two that resolve point at *moved* files anyway (the real tests live under
  `crates/cargonaut-bin/tests/` and `crates/cargonaut-transfer/tests/`); the manifest's
  paths (`tests/integration/resume_sigkill.rs`, `tests/integration/cancellation.rs`) are
  the dead root locations.
- File header asserts: *"CI greps this file to verify every requirement has a linked
  test or manual check."* Grep for `requirements.toml` across `*.sh`, `*.yml`,
  `Makefile`, `*.rs` (excluding `target/`) → **zero references.** The claim is false.
- The manifest mirrors `design/spec.md` (the original 6-phase master plan, FR-001…FR-503),
  which predates the per-feature `specs/NNN/` workflow adopted at feature 031.

**Rationale**: A file that (a) nothing reads and (b) misrepresents 97% of its links is
worse than no file — it manufactures false confidence in traceability. The current
source of truth is each `specs/NNN/` feature's own spec + tests.

**Alternatives considered**:
- *Reconcile all 57 paths to current tests* — rejected: disproportionate archaeology
  for a superseded document nothing consumes; many original FRs were implemented
  differently or deferred across 27 features, so a faithful remap is large and
  low-value.
- *Delete the file outright* — rejected (soft): the project's `CLAUDE.md` deferral rules
  and constitution value durable paper trails; the original requirements record has
  historical worth. Git history would preserve it on delete, but an in-tree banner is
  more discoverable. **Chosen: archive in place with a banner + remove the false claim.**

## R-002 — Which files in `design/contracts/` are live vs historical?

**Decision**: Only `requirements.toml` is historical; everything else stays untouched.

**Evidence**:
- `design/contracts/` contains: `requirements.toml` (historical), `config.schema.json`
  (modified Jun 19, consumed by config validation), `keymap.toml` (modified Jun 20),
  `commands.toml`, `menu.schema.json`, `openers.schema.json`, `plugin-api.md`.
- **Constitution §III** names `design/contracts/keymap.toml` as the single source of
  truth for keybindings, *"loaded at startup."* It is constitutionally authoritative —
  must not be touched.

**Rationale**: The directory mixes one dead file with several live ones. Surgical
archiving (one file) avoids collateral damage to authoritative config.

**Alternatives considered**: *Move all of `design/` to an `archive/` tree* — rejected:
would relocate the live `keymap.toml`/schemas and break the constitution's path
reference and any config loader expectations.

## R-003 — Is the `Cargo.toml` header genuinely stale?

**Decision**: Yes; rewrite the comment to point at the live workflow.

**Evidence**: Header reads *"See design/plan.md … `cargo build` produces the cargonaut
binary (Phase 1 in progress)."* The repo has merged feature 057 (`git log`), and the
authoritative pointer is `.specify/feature.json` → `specs/NNN/` (+ `CLAUDE.md`). The
comment points readers at the very artifact being archived in R-001.

**Rationale**: `Cargo.toml` is the first file most Rust readers open; its orientation
text should be correct. Comment-only change → no build impact.

**Alternatives considered**: *Delete the comment* — rejected: a short, correct pointer
is more useful than none.

## R-004 — Are the root `tests/integration/` and `benches/` safe to delete?

**Decision**: Yes; delete both.

**Evidence**:
- `tests/integration/`: 0 entries on disk; `git ls-files tests/` → empty (untracked).
  Removal is a working-tree-only action with no committed diff.
- `benches/`: contains only `benches/.gitkeep` (tracked; added in the initial scaffold
  commit `90ad047`). All 11 real benches live under `crates/*/benches/` with
  `harness = false`.
- Build references: `Cargo.toml` has no root `benches`/`tests` globs; `Makefile`
  `bench:`/`test:` targets don't reference the root locations; `ci.yml` mentions
  `benches/*.rs` only in a *comment*. Spec-doc mentions of `benches/foo.rs` are
  conceptual shorthand for the per-crate benches, not build inputs.

**Rationale**: Empty/placeholder dirs imply a layout the project doesn't use; removing
them makes the root honest.

**Alternatives considered**: *Keep `benches/.gitkeep` as a future placeholder* —
rejected: per-crate benches are the established pattern; a root placeholder is
misleading, not aspirational.

## R-005 — docs-gate interaction

**Decision**: Satisfy the gate honestly by updating `README.md` + `Learnings.md`.

**Evidence**: `scripts/ci/docs-gate.sh` rejects `NNN-name` PRs that don't modify both
files unless a commit message contains `[no-docs]`. This change genuinely alters how the
repo is navigated (archived manifest, corrected pointers), so it qualifies as a feature
update, not an infra-only bypass.

**Rationale**: Using `[no-docs]` here would understate a change that affects contributor
navigation. Update both docs.

## R-006 — The deferred `cargonaut-core` god-file (out of scope, tracked)

**Decision**: Do **not** implement here; record as a GitHub issue + ROADMAP row.

**Evidence**: `crates/cargonaut-core/src/lib.rs` = 6,246 lines, ~300 `fn` defs, **1**
`mod` declaration — effectively one giant module. Contrast `cargonaut-ui-tui` (6
submodules) and `cargonaut-vfs` (`archive/`, `remote/` dirs).

**Rationale**: A module split is a behavior-preserving refactor large enough to warrant
its own feature/spec; bundling it into a housekeeping PR would violate the "no production
code changes" boundary and muddy review. `CLAUDE.md` mandates a paper trail (issue +
ROADMAP) for such deferrals.

**Effort estimate (for the follow-up)**: M — mechanical but wide; split `lib.rs` into
cohesive submodules (e.g. `fs`, `compare`, `rename`, `history`, `attrs`), keep the public
re-export surface stable, verify clippy + tests unchanged.
