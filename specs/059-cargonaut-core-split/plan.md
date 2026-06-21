# Implementation Plan: cargonaut-core God-File Split

**Branch**: `059-cargonaut-core-split` | **Date**: 2026-06-21 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/059-cargonaut-core-split/spec.md`

## Summary

`crates/cargonaut-core/src/lib.rs` is a 6,246-line single module (≈2,700 lines production + ≈3,537 lines tests). This feature splits it into 12 cohesive production submodules + 1 test-support module, leaving `lib.rs` a thin module root (~150–230 lines: crate docs, imports, `mod` declarations, the `pub use` surface, and the two structs with private fields — `App` and `SideState`). The refactor is **move-only**: no logic, signature, control-flow, or message changes. Public-API stability is proven two ways — a rustdoc-JSON surface diff against a committed 179-line baseline, and compiling/testing every downstream crate **and the benches** with zero source edits. See [research.md](./research.md) for the decisions; [data-model.md](./data-model.md) for the module map; [contracts/](./contracts/) for the API gate.

## Technical Context

**Language/Version**: Rust, edition 2021, `rust-version = 1.76`; dev toolchain is nightly 1.97 (default) — used only for rustdoc-JSON API extraction, not required to build.

**Primary Dependencies**: Internal — `cargonaut-vfs`, `cargonaut-transfer`, `cargonaut-config`. External — `tokio`, `globset`, `thiserror`, `serde` (unchanged by this feature).

**Storage**: N/A (no runtime data introduced; hotlist persistence path logic is moved, not changed).

**Testing**: `cargo test --workspace` (in-crate unit tests relocated alongside their modules); `cargo bench` (criterion benches in `cargonaut-core/benches/` must keep compiling against the re-exported surface).

**Target Platform**: Linux (TUI file manager).

**Project Type**: Rust workspace / library crate refactor (single crate touched: `crates/cargonaut-core`).

**Performance Goals**: No change. Move-only refactor must not perturb the four §IV benches (SC-001/002/003/004) or NFR-001/002; verified by the benches still building and CI bench gates staying green.

**Constraints**: Public API byte-for-byte stable (FR-003); zero downstream `src/` edits (FR-004); no visibility widening beyond crate-internal where strictly required (FR-007); `#![warn(missing_docs)]` + `-D warnings` + `-D broken-intra-doc-links` stay clean (FR-008/FR-009); SSD/tmpfs discipline (Constitution §V).

**Scale/Scope**: One 6,246-line file → ~14 files. 179-item public surface to preserve. ~70 `App`/value-type methods + 12 free fns + ~140 test fns to relocate.

## Constitution Check

*GATE: evaluated before Phase 0 and re-checked after design. Version 1.1.0.*

| Principle | Status | Notes |
|-----------|--------|-------|
| **I. Code Quality** | ✅ PASS | `-D warnings`, `#![warn(missing_docs)]`, `fmt --check`, `-D broken-intra-doc-links` all remain enforced; the split adds no `unsafe`. Move-only, so no new lint surface beyond mechanical `use` paths. |
| **II. Test-First** | ⚠️ JUSTIFIED DEVIATION | No new behavior → no new red-first tests. The existing green suite is the regression guard; the new `contracts/` surface diff is the added contract smoke gate. Allowed by §II's no-behavior clause. See Complexity Tracking. |
| **III. UX Consistency** | ✅ N/A | No TUI/keymap/theme changes; this crate is headless state/dispatch. |
| **IV. Performance** | ✅ PASS | Move-only; benches unchanged and must keep compiling/passing (R-005 consumer proof includes benches). No >10% regression possible from relocation. |
| **V. SSD Preservation** | ✅ PASS | tmpfs symlink active (verified); use `make` wrappers; no `cargo clean`/`rm -rf target`. |

**Quality Gates (per-PR)**: fmt, clippy, build, test, doc-build (strict links), binary-size, coverage ≥80% — all run by `make ci-local`. Coverage cannot drop: tests are relocated, not removed (FR-006).

**Gate result**: PASS with one documented, principle-sanctioned deviation (§II). No unjustified violations.

## Project Structure

### Documentation (this feature)

```text
specs/059-cargonaut-core-split/
├── plan.md              # This file
├── spec.md              # Feature specification
├── research.md          # Decisions R-001..R-008
├── data-model.md        # Module decomposition map (code-org entities)
├── quickstart.md        # Validation guide (how to prove it worked)
├── contracts/
│   ├── extract-public-api.py     # rustdoc-JSON → normalized surface
│   ├── public-api-baseline.txt   # 179-line pre-refactor surface (the gate)
│   └── README.md                 # how to run the API-stability check
├── checklists/
│   └── requirements.md           # spec quality checklist (done)
└── tasks.md             # Phase 2 output (/speckit-tasks)
```

### Source Code (repository root)

```text
crates/cargonaut-core/src/
├── lib.rs          # THIN module root: docs, use, `mod` decls, `pub use` surface,
│                   #   `App` struct, `SideState` struct  (~150–230 lines)
├── pane.rs         # PaneId, PaneFilter, PaneState, FocusedRow, TabBarEntry,
│                   #   ViewMode, SplitOrient (+impls); pane_idx; glob_match
├── command.rs      # Command, Event, DialogKind
├── error.rs        # AppError, UndoEntry
├── jobs.rs         # JobStatus, JobView, ProgressView, ResumeOfferView +
│                   #   transfer_state_snapshot, job_status_from, resume_offer_view, crc32_partial
├── app.rs          # impl App: new, accessors, dispatch (the router)
├── nav.rs          # impl App: navigation, cwd/listing, set_filter; parse_path, sort helpers
├── history.rs      # impl App: history_prev_dir / history_next_dir
├── fsops.rs        # impl App: mkdir, select_by_pattern, recursive_dir_size
├── attrs.rs        # impl App: chmod/chown (+recursive), links, subtree walk; status helpers
├── compare.rs      # impl App: compare_directories
├── rename.rs       # impl App: undo_last_operation, apply_bulk_rename; validate_rename_proposals
├── hotlist.rs      # impl App: bookmarks / add / remove / jump / persist
├── tabs.rs         # impl App: tab_new/close/next/prev, tab_bar_view
├── transfers.rs    # impl App: transfers, resume offers, confirmations
└── test_support.rs # #[cfg(test)] pub(crate): make_app, app_with_three, fixtures
                    #   (each src module also gains its own #[cfg(test)] mod tests)
crates/cargonaut-core/benches/   # UNCHANGED — must still compile (API consumer proof)
crates/cargonaut-{ui-tui,transfer,bin}/src/   # UNCHANGED (FR-004)
```

**Structure Decision**: Flat top-level submodules under `src/` (not nested under an `app::` parent). Justification in research.md R-002/R-003: `App` and `SideState` stay in the crate-root `lib.rs` so all flat submodules are descendant modules and retain private-field access with **zero** visibility widening; flat naming maximizes navigability over an artificial `app::` nesting.

## Implementation Phasing (preview — detailed in tasks.md)

1. **Baseline & guard**: confirm baseline surface (committed), confirm green starting state (`make test`, clippy).
2. **Leaf types out** (no `App` dependence): `error`, `command`, `jobs`, `pane` — re-export from `lib.rs`, keep green after each.
3. **Core out**: `app.rs` (router + accessors); `App`/`SideState` stay in `lib.rs`.
4. **Method modules out**: `nav`, `history`, `fsops`, `attrs`, `compare`, `rename`, `hotlist`, `tabs`, `transfers` — one `impl App` block each.
5. **Tests out**: `test_support` + per-module `#[cfg(test)] mod tests`; `lib.rs` test block emptied.
6. **Verify & ship**: API surface diff empty; `make ci-local` green; benches build; README + Learnings; close #86; reconcile ROADMAP.

Each step: move → `cargo fmt` → `make test` + `cargo clippy --workspace --all-targets -- -D warnings` → commit. Order respects R-008.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| §II red-first cadence not followed | This is a move-only, behavior-preserving refactor (FR-005). Authoring failing tests for code that already exists and already passes would be theater — the relocated tests *are* the red/green evidence, and they were red→green in their original feature branches. | Writing new failing tests first would require deleting then re-adding working code, adding risk and noise for zero behavioral coverage gain. §II explicitly permits no-behavior passes to ship without red-first when a contract smoke test exists — satisfied by the existing API-stability guard test plus the new `contracts/` surface diff. |
| ~3 crate-internal `pub(crate)` widenings possible (`pane_idx`, `parse_path`, shared status helpers) *if* a helper is referenced across module boundaries | Some private free fns are called from more than one new module; the narrowest fix is `pub(crate)`, not `pub`. | Duplicating the helper per module would violate move-only (DRY divergence) and risk drift; making it `pub` would change the public surface (forbidden, FR-003). `pub(crate)` is invisible to other crates and to the public-API gate — the narrowest legal scope (FR-007). |
