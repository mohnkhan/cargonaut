# Research: cargonaut-core God-File Split

**Feature**: 059-cargonaut-core-split | **Date**: 2026-06-21

This refactor has no unknown *product* requirements — the spec is fully determined. The open questions are all **technical/mechanical**: how to split a 6,246-line Rust module while provably preserving the public API and all behavior. Each decision below is grounded in the actual contents of `crates/cargonaut-core/src/lib.rs` (measured 2026-06-21).

## File anatomy (measured)

| Region | Lines | Content |
|--------|-------|---------|
| Header + docs + `use` + re-exports | 1–38 | crate docs, imports, `pub use` of `TransferId`/`TransferMode`/`Bookmark`/`Hotlist` |
| Value types | 39–610 | `PaneId`, `PaneFilter`, `PaneState`, `FocusedRow`, `SideState`, `TabBarEntry`, `Command`, `ViewMode`, `ProgressView`, `JobStatus`, `JobView`, `ResumeOfferView`, `SplitOrient`, `Event`, `DialogKind`, `AppError`, `UndoEntry` + their inherent impls |
| `App` struct | 618–648 | the central state type (12 **private** fields) |
| `impl App` #1 | 649–2224 | constructor, accessors, `dispatch`, navigation, fs ops, attrs, recursive walk, links, bookmarks, filter, history, confirmations, compare |
| `impl App` #2 | 2224–2494 | tab ops, undo, bulk rename |
| Free functions | 2494–2708 | `validate_rename_proposals` (pub), `glob_match` (pub), `transfer_state_snapshot` (pub) + 9 private helpers |
| `#[cfg(test)] mod tests` | 2709–6246 | **3,537 lines** — over half the file |

**Public surface** (rustdoc-JSON baseline, `contracts/public-api-baseline.txt`): 179 items — 41 `App` methods, 7 `PaneState` methods, 3 `PaneFilter`, 1 each `PaneId`/`SplitOrient`/`ViewMode`; 7 structs, 10 enums (73 variants), 28 public fields, 3 free fns, 4 re-exports.

## R-001 — Splitting one `impl App` across files

**Decision**: Distribute `App`'s methods into per-responsibility submodules, each containing its own `impl App { … }` block. Rust permits an unlimited number of inherent `impl` blocks for a type, in any module of the defining crate.

**Rationale**: The bulk of the file is `App` methods, not type definitions. Multiple `impl App` blocks is the standard, zero-cost idiom for this. Method resolution is on the type, so `dispatch` (in one module) calls `self.compare_directories()` (in another) with no ceremony.

**Alternatives considered**: Extension traits (`trait NavOps { … } impl NavOps for App`) — rejected: adds a public/!public trait surface decision, risks changing the API, and requires importing the trait at call sites. Free functions taking `&mut App` — rejected: changes call syntax and forces field widening.

## R-002 — Private-field visibility (the crux)

**Decision**: Keep **only** `App` and `SideState` defined in `lib.rs` (the crate root). Move `PaneState`, every other type, every `impl` block, every free fn, and all tests into submodules. **No visibility modifier changes anywhere.**

**Rationale**: Rust privacy: a private field is visible in the module that defines the struct *and all descendant modules*. `lib.rs` is the crate-root module, so **every** submodule is a descendant and can read `App`/`SideState` private fields. Therefore relocating `impl App` methods into submodules needs **zero** `pub`/`pub(crate)` additions. Measurement confirms the two relevant facts:
- `App`'s 12 fields are all private → must stay reachable by all method modules → keep `App` at root.
- `SideState`'s 2 fields (`tabs`, `active_tab`) are private and touched by `App` methods → keep `SideState` at root.
- `PaneState`'s 10 fields are **all `pub`** → `PaneState` can move to a submodule with no friction.

This is the best possible outcome for **FR-007** (no widening) and keeps the change maximally move-only.

**Alternatives considered**:
- Move `App` into a `state` module and mark its fields `pub(crate)`: gives a slightly thinner `lib.rs` but introduces ~14 visibility widenings (crate-internal, but still a deviation from move-only). Rejected — keeping two small structs at root costs ~45 lines and buys zero churn.
- Make submodules children of an `app::` module so they stay descendants of a relocated `App`: works, but nesting unrelated concerns (`app::compare`, `app::tabs`) under `app::` is less navigable than flat top-level modules. Rejected.

**Consequence**: `lib.rs` retains crate docs, imports, re-exports, `mod` declarations, the `pub use` surface, and the `App` + `SideState` definitions (~45 lines of struct) → estimated **150–230 lines** total. Well within "thin module root" (SC-001).

## R-003 — Module decomposition map

**Decision**: 12 production submodules + 1 test-support module, grouped by the feature-history banners already present in the file (the code is *already* informally sectioned by `// ===== Feature NNN =====` comments — we formalize those seams).

| Module | Owns (types) | Owns (`impl App` / fns) |
|--------|--------------|--------------------------|
| `pane` | `PaneId`, `PaneFilter`, `PaneState`, `FocusedRow`, `TabBarEntry`, `ViewMode`, `SplitOrient` + their impls | `pane_idx` helper |
| `command` | `Command`, `Event`, `DialogKind` | — |
| `error` | `AppError`, `UndoEntry` | — |
| `jobs` | `JobStatus`, `JobView`, `ProgressView`, `ResumeOfferView` | `transfer_state_snapshot` (pub), `job_status_from`, `resume_offer_view`, `crc32_partial` |
| `app` | — | `new`, all accessors, `dispatch` (the router) |
| `nav` | — | navigation + cwd/listing: `relist_active`, `navigate_into`, `refresh_active_pane`, `descend_into_focused`, `sync_other_panel_path`, `show_focused_in_other_panel`, `ascend_to_parent`, `navigate_to`, `resolve_cd_target`, `quick_cd`, `complete_cd`, `selection_or_focused`, `set_filter`; `parse_path`, `next_sort_key`, `sort_label` |
| `history` | — | `history_prev_dir`, `history_next_dir` |
| `fsops` | — | `mkdir`, `select_by_pattern`, `recursive_dir_size` |
| `attrs` | — | `chmod_selection`, `chown_selection`, `collect_subtree`, `collect_subtree_capped`, `chmod_recursive`, `chown_recursive`, `attr_roots`, `create_symlink`, `create_hard_link`, `link_source`; `RECURSE_NODE_CAP`, `recursive_status`, `attr_status` |
| `compare` | — | `compare_directories` |
| `rename` | — | `undo_last_operation`, `apply_bulk_rename`; `validate_rename_proposals` (pub) |
| `hotlist` | — | `bookmarks`, `add_bookmark`, `remove_bookmark`, `jump_to_bookmark`, `persist_hotlist` |
| `tabs` | — | `tab_new`, `tab_close`, `tab_next`, `tab_prev`, `tab_bar_view` |
| `transfers` | — | `transfer_ids`, `transfer`, `job_views`, `cancel_transfer`, `pause_transfer`, `resume_paused`, `confirm_copy`, `transfer_opts`, `scan_resume_offers`, `pending_resume_views`, `resume_offer`, `start_over_offer`, `skip_offer`, `request_copy_confirmation`, `request_move_confirmation`, `request_delete_confirmation` |
| `glob_match` (pub) | — | placed in `pane` (filter-adjacent) **or** a tiny `util` module |

This is 12–13 modules. The spec floor is ≥4 (FR-002); we exceed it to maximize cohesion. Each module's responsibility is stateable in one sentence (SC-002, SC-007). `glob_match` placement is the only soft call — folded into `pane` to avoid a one-function module.

**Rationale**: Boundaries follow the existing `// ===== Feature NNN =====` banners, so the split is recognizable to anyone who knows the feature history, and review locality maps to how the code grew.

## R-004 — Test relocation

**Decision**: Co-locate each test group as a `#[cfg(test)] mod tests` inside the submodule whose code it exercises. Shared fixtures (`make_app`, `app_with_three`, `mode_of`, `entry_index`, `submit_one_copy`) move into a `#[cfg(test)] pub(crate) mod test_support` so every module's tests can `use crate::test_support::*`.

**Rationale**: The monolithic 3,537-line test block is the larger half of the god-file; leaving it whole in `lib.rs` would violate FR-010 (no resulting god-file) and SC-006. The tests already self-label by feature (`// ===== Feature 050 T007 … =====`), so they partition along the same module seams. Unit tests remain in-crate, so they keep access to private methods/fields they exercise (e.g. `collect_subtree`, `default_cursor`). `pub(crate)` on the test-support module is `#[cfg(test)]`-gated → invisible to non-test builds and to other crates, so it does not affect the public surface.

**Alternatives considered**: Convert to `tests/` integration tests — rejected: those only see the public API and would lose access to private helpers many tests rely on, forcing artificial visibility widening (violates FR-007). Keep one central test module — rejected per FR-010.

## R-005 — API-stability verification (the gate)

**Decision**: Two independent, complementary proofs.
1. **Surface diff (automated)**: `contracts/extract-public-api.py` renders the rustdoc-JSON public surface to a normalized, sorted text file. The committed baseline is `contracts/public-api-baseline.txt` (179 lines, captured pre-refactor). After the split, regenerate and `diff` against the baseline — **must be empty**.
2. **Consumer compile/test proof (authoritative)**: build + test the whole workspace, **plus the `cargonaut-core` benches** (`benches/{startup,compare_dirs,rss_headroom,bulk_rename}.rs`), with **zero edits** to any downstream `src/`. Benches are an extra public-API consumer the issue did not mention but that the constitution's Performance gate depends on — they must still compile against the re-exported surface.

**Rationale**: The surface diff catches additions/removals/renames at name granularity; the compile proof catches anything name-granularity misses (signature drift, trait-impl changes, inference breaks at real call sites). Together they bracket "public API unchanged" from both the producer and consumer side. `cargo public-api` would be a nicer single tool but is **not installed**; the rustdoc-JSON approach needs only the (default) nightly toolchain and Python, both present. `jq` is **absent**, so the extractor is Python.

**Note**: rustdoc emits one pre-existing `private_intra_doc_links` warning for `cargonaut-core` (observed during baseline generation). It is unrelated to this work and not introduced by it; the docs gate the PR must pass is `RUSTDOCFLAGS="-D rustdoc::broken-intra-doc-links"` (Constitution §I), a different lint. We will confirm no *new* doc warnings appear.

## R-006 — Test-First constitution principle for a move-only refactor

**Decision**: Treat the **existing, already-green test suite as the regression guard**; do not author new red tests for relocated behavior. Each move commit must leave `cargo test --workspace` + `cargo clippy -D warnings` green.

**Rationale**: Constitution §II (Test-First) requires a red→green history for *new behavior* (FR-### that change runtime semantics). This feature adds **no behavior** — FR-005 forbids logic/signature/control-flow change. §II explicitly allows no-behavior passes to "ship in a single commit, provided they still add an object-safety or contract smoke test." Here the "contract smoke test" already exists in two forms: the Feature 053 *"API stability regression guard"* test (in the current test module) and this feature's new `contracts/` surface gate (R-005), which becomes a CI-checkable artifact. No SC introduces a new runtime metric, so no new bench/gate is owed under §II.

**Consequence**: This is recorded as an intentional, principle-compliant deviation from the literal red-first cadence, justified in plan.md §Constitution Check.

## R-007 — Build hygiene (SSD / tmpfs / CI)

**Decision**: Use the `make` wrappers (`make build`, `make test`, `make ci-local`) which enforce `make check-tmpfs`. `make tmpfs-status` confirms `target → /tmp/cargonaut/475f93e39f14/target` is active (verified 2026-06-21, 763 MB in tmpfs, 40% of 16 G). Never run `cargo clean` / `rm -rf target` (Constitution §V). Final gate before PR: `make ci-local` (clippy → test → release build → check-pr-body → docs-gate).

**Rationale**: Constitution §V is non-negotiable on the dev host; the symlink is already set up, so no `make tmpfs-setup` needed.

## R-008 — Commit granularity

**Decision**: One module-extraction per commit, in dependency-safe order: leaf value types first (`error`, `command`, `jobs`, `pane`), then `app` (struct stays in lib; router + accessors out), then the method modules (`nav`, `history`, `fsops`, `attrs`, `compare`, `rename`, `hotlist`, `tabs`, `transfers`), then `test_support` + per-module test relocation. Run `make test` + clippy after each. The issue (§Suggested approach) calls for exactly this "small, reviewable commits; prove no behavior change after each."

**Rationale**: Small reversible steps keep every intermediate state compiling and green, make review tractable, and isolate any accidental behavior change to a single move.

## Open risks & mitigations

| Risk | Mitigation |
|------|-----------|
| A relocated method needs a private helper now in a different module | Helpers move *with* their primary caller (see R-003 placement); cross-module private fns become `pub(crate)` only if genuinely shared (e.g. `pane_idx`, `parse_path`) — a crate-internal widening, never public (FR-007 allows narrowest-scope crate-internal). |
| Test helper used by tests in many modules | Centralized in `test_support` (R-004). |
| Intra-doc link breaks when an item moves | Links are crate-path-resolved (`[\`App::foo\`]`); they survive moves. Docs gate (`-D broken-intra-doc-links`) in `make ci-local` confirms. |
| `lib.rs` still too large because `App` impls accidentally left behind | SC-001 line-count check + `grep -c '    fn ' lib.rs` ≈ 0 after split. |
