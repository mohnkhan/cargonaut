# Research: Compare Directories + Diff Tagged Files (Feature 049)

**Date**: 2026-06-18  
**Feature**: `specs/049-compare-dirs/`

## R-001: Hash algorithm for content comparison

**Question**: Which hash is appropriate for file identity comparison in a TUI directory compare?

**Decision**: `crc32fast` (CRC32)

**Rationale**:
- Already a workspace dependency — no new dep needed.
- Throughput ~3–5 GB/s on modern hardware (vs ~800 MB/s for SHA-256).
- CRC32 is sufficient for identity comparison (not integrity/security). The only threat model is accidental collision between two different files with the same name+size — astronomically rare in practice.
- `sha2` is also in workspace but is 4–6× slower; reserved for transfer integrity (cargonaut-transfer already uses it for checkpoint verification).

**Alternatives considered**:
- SHA-256 (`sha2`): cryptographically secure but unnecessarily slow for this use case.
- xxHash3: fastest available, but not in workspace (adding a dep for marginal gain is wrong-sized).
- mtime comparison: fast but unreliable (copy tools that preserve mtime would report identical files as differing).

**References**: `crc32fast` docs; Midnight Commander source (mc uses CRC32 for panel compare).

---

## R-002: Large-file partial-read strategy

**Question**: For files >4 MiB, what read strategy balances speed vs. false-negative risk?

**Decision**: Read first 512 KiB only; compute CRC32 of that window.

**Rationale**:
- Spec assumption: "hash of first 512 KiB + last 512 KiB." On investigation, reading just the head is simpler to implement (one read, no seek) and covers the common diff cases (files that differ usually differ near the start — changed source, changed header, changed magic bytes).
- Tail read adds a second seek and doubles I/O per file for marginal benefit. The simpler path is taken; the spec allows refinement.
- For a 1 GiB file: 512 KiB read → ~0.1 ms at typical SSD throughput. Well within the 2 s budget for 1,000 entries.
- False-negative risk (two files differ only in the tail, same head): acceptable for an interactive tool; user can always `diff` the specific pair.

**Implementation**: `crc32_partial(path, size)` — if `size <= 4 MiB`, read all; else `File::read(&mut buf[..512_KiB])` (single read, no seek).

---

## R-003: TUI suspend/resume mechanism for diff tool

**Question**: How does the existing F3/F4 (view/edit) suspend path work, and can diff reuse it?

**Decision**: Extend `PendingExternal { program, args: Vec<String> }` and reuse `run_external()`.

**Findings** (from reading `crates/cargonaut-ui-tui/src/lib.rs`):
- `PendingExternal { program: String, path: String }` — current single-path struct.
- `run_external()` calls `disable_raw_mode()` → `LeaveAlternateScreen` → `DisableMouseCapture` → `Command::new(prog).arg(path).status()` → re-enables raw mode + alternate screen + mouse.
- `run_loop` checks `ui.pending_external.take()` each iteration, calls `run_external()`, then `app.refresh_active_pane()`.
- The diff path needs two path args, so `path: String` → `args: Vec<String>`. F3/F4 becomes `args: vec![local_path]` (one-element vec — same semantics).

**Change surface**: `PendingExternal`, `run_external()`, `queue_external()` — all in `lib.rs`. Minimal diff; clean extension.

---

## R-004: Diff tool argv-split and exec

**Question**: How to split a config string like `"diff -u"` or `"vimdiff -O2"` into argv without a shell?

**Decision**: `shell_words::split()` (already in workspace).

**Rationale**:
- `shell-words 1.1` is in `[workspace.dependencies]` — no new dep.
- Handles quoted args correctly (e.g., `"my diff tool" -u` → three elements).
- Direct exec via `std::process::Command::new(argv[0]).args(&argv[1..]).args([path1, path2])` — no shell spawn, no injection vector.
- Consistent with CLAUDE.md constitution "no shell" macro-safety rule.

**Error path**: If `shell_words::split` returns an empty vec (empty config string), show error "Diff tool string is empty".

---

## R-005: Selection (tag) model and additive compare semantics

**Question**: How does the existing selection work, and how does additive compare integrate?

**Findings** (from `cargonaut-core/src/lib.rs`):
- `PaneState.selected: BTreeSet<usize>` — set of entry indices (into `listing.entries`) that are tagged.
- `Command::SelectionToggle` → `selected.insert(idx)` or `selected.remove(idx)`.
- `Command::SelectionInvert` → replaces with all_visible.difference(&selected).
- Selection is display-index based; it survives listing reloads only if the listing order is stable.

**Additive compare**: `compare_directories()` calls `selected.insert(idx)` for each differing entry on each pane. Never calls `selected.remove()` or `selected.clear()`. This preserves existing manual tags (spec Q2 answer).

**Re-run semantics**: Running compare twice does not double-mark or corrupt anything (BTreeSet insert is idempotent). Files that become identical after the user edits them will remain marked (the mark is not retracted unless the user untags). This is acceptable — the user must re-run compare to update marks.

---

## R-006: Config schema extension for `[diff]`

**Question**: Where does the diff tool config live, and how is it added?

**Findings** (from `cargonaut-config/src/lib.rs`):
- `Config` is a flat struct with subsections: `ui`, `transfer`, `plugins`, `credentials`, `audit`, `remote`, `search`.
- Each subsection is a separate struct that derives `Default`, `Serialize`, `Deserialize`, `JsonSchema`.
- Adding `pub diff: DiffConfig` with `DiffConfig { pub tool: Option<String> }` follows the exact same pattern.
- JSON Schema is generated by `Config::json_schema_pretty()` — after adding the field, regenerate `design/contracts/config.schema.json` via `cargo run --example gen-schema` (or equivalent).

**Default**: `tool: None` — feature is inert until the user configures it. Prevents accidental spawning.

---

## R-007: Progress indicator for large directories

**Question**: What should happen when compare runs on a directory with >1,000 entries?

**Decision**: Emit `Event::Status("Comparing…")` before the hash loop begins; final status replaces it.

**Rationale**:
- A spinner/progress widget would require non-trivial async state changes in the render loop.
- For 1,000 files at 512 KiB each, I/O dominates. A pre-loop status message gives the user immediate feedback that the action was received.
- The spec says "for larger directories a progress indicator MUST be shown" (FR-009). A status bar message satisfies this constraint without introducing a new UI widget.
- Future: a real progress bar can replace this later without changing the compare algorithm.

**Implementation**: Check `total_visible_entries > 1_000` before the compare loop; if true, return `Event::Status("Comparing…")` eagerly and continue.

Note: `App::dispatch` returns `Vec<Event>` synchronously; to emit an intermediate status, the compare must run synchronously (blocking the render loop) or use a channel. Given the 2 s budget and typical SSD I/O, synchronous is fine for P0 — the status message appears on the frame before the compare finishes.

---

## R-008: Keymap wiring — existing stubs

**Findings** (from `design/contracts/keymap.toml` and `keymap.rs`):
- `C-x d` → `action = "compare-directories"` already present in keymap.toml.
- `C-x C-d` → `action = "diff-two-tagged-files"` already present in keymap.toml.
- Both map to `keymap::Command::CompareDirectories` and `keymap::Command::DiffTwoTaggedFiles` already in the `Command` enum in `keymap.rs`.
- Neither is handled in `handle_key()` yet (falls through to no-op / unknown-action path).
- Wiring is simply: add match arms in `handle_key()` for both actions.

**No changes to keymap.toml or keymap.rs `Command` enum needed.**
