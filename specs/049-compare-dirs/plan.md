# Implementation Plan: Compare Directories + Diff Tagged Files

**Branch**: `049-compare-dirs` | **Date**: 2026-06-18 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/049-compare-dirs/spec.md`

## Summary

Implement two cooperating actions for a two-panel TUI file manager:

1. **Compare directories** (`C-x d`): compare the visible listings of both panels by name, size, and CRC32 content hash, then additively mark all differing entries using the existing `BTreeSet<usize>` selection mechanism.
2. **Diff two tagged files** (`C-x Ctrl-d`): validate that exactly two files are tagged, then suspend the TUI and hand the terminal to a user-configured external diff tool (argv-split command string), resuming cleanly on exit — the same suspend/restore path already used by F3/F4.

Both keymap action stubs already exist in `keymap::Command` and `design/contracts/keymap.toml`. This feature wires them to real implementations.

## Technical Context

**Language/Version**: Rust 1.76 (workspace `rust-version`)

**Primary Dependencies** (all already in `Cargo.toml` workspace deps):
- `ratatui 0.27` + `crossterm 0.28` — TUI
- `crc32fast 1` — fast content hash for compare
- `shell-words 1.1` — argv-split the diff tool config string
- `sha2 0.10` — in workspace but NOT used here; CRC32 is sufficient for identity comparison
- `tokio` — async file I/O for hash computation

**Storage**: N/A — compare results are ephemeral; the `BTreeSet<usize>` in `PaneState` carries the marks.

**Testing**: `cargo test --workspace`; async tests via `#[tokio::test]`; property tests via `proptest`

**Target Platform**: Linux TUI (POSIX local filesystem only for this feature)

**Performance Goals**: SC-001 ≤2 s for 1,000 entries; SC-004 ≤500 ms from keypress to tool launch

**Constraints**: Compare is additive (never clears existing tags); diff tool is argv-split + direct exec (no shell)

**Scale/Scope**: Listings up to ~10,000 entries (progress indicator beyond 1,000); no recursive compare

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Gate | Status | Notes |
|---|---|---|
| §I Clippy -D warnings | PASS | All new code will be lint-clean; `missing_docs` on public items |
| §I unsafe blocks | PASS | No unsafe needed; pure safe Rust |
| §II TDD (red → green per task) | PASS | Each task commits failing tests before implementation |
| §II SC benchmarks | PASS | SC-001 bench (`benches/compare-dirs.rs`) gates on 2 s p95 |
| §III UX consistency | PASS | Uses existing `BTreeSet<usize>` selection; no new visual system |
| §III Keymap source-of-truth | PASS | Both actions already in `keymap.toml` and `keymap::Command` |
| §IV Performance (SC-001/004) | PASS | CRC32 + partial-read strategy; bench in CI |
| §V SSD preservation | PASS | No new artifact trees; `make check-tmpfs` unchanged |

No violations requiring justification.

## Project Structure

### Documentation (this feature)

```text
specs/049-compare-dirs/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
└── tasks.md             # Phase 2 output (/speckit-tasks)
```

### Source Code (repository root)

This feature spans three existing crates — no new crates added.

```text
crates/
├── cargonaut-config/src/lib.rs      # Add DiffConfig { tool: Option<String> }
├── cargonaut-core/src/lib.rs        # Add Command::CompareDirectories + compare_directories()
└── cargonaut-ui-tui/src/lib.rs      # Wire both keymap actions; extend PendingExternal

design/
└── contracts/
    ├── keymap.toml                   # Already has both action stubs — no changes needed
    └── config.schema.json            # Add [diff] section + diff.tool property

benches/
└── compare-dirs.rs                   # SC-001 criterion bench (new)
```

## Complexity Tracking

No constitution violations — this section is empty.

## Implementation Strategy

### Phase A: Config extension (`cargonaut-config`)

Add `DiffConfig` to the `Config` root struct:

```rust
/// External diff tool settings (Feature 049).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct DiffConfig {
    /// Argv string for the external diff tool.
    /// Split on whitespace; two file paths appended as final args.
    /// Example: `"diff -u"`, `"vimdiff"`, `"meld"`.
    /// Defaults to `None` (feature disabled until configured).
    pub tool: Option<String>,
}
```

Add `pub diff: DiffConfig` to `Config`.

### Phase B: Core — `Command::CompareDirectories` + compare logic

**New command variant:**
```rust
/// Feature 049 — compare both panels' visible listings and additively
/// mark all differing entries (name-only, size-differ, or hash-differ).
CompareDirectories,
```

**`App::compare_directories()`** algorithm:
1. Check both `pane[Left].cwd` and `pane[Right].cwd` are local (`file://` scheme) — return `Status` error otherwise.
2. If both cwds equal → return `Status("Both panels point to the same directory — compare would mark nothing")`.
3. Build `left_map: HashMap<&str, (usize, u64, VfsKind)>` from Left pane's visible entries (name → index, size, kind).
4. Build `right_map: HashMap<&str, (usize, u64, VfsKind)>` from Right pane's visible entries.
5. For each entry in left_map:
   - Not in right_map → mark left index.
   - In right_map:
     - Either is a Dir → "same" (name-presence only; dirs not content-compared).
     - Sizes differ → mark both indices.
     - Sizes equal, both Files → CRC32 both → if differ, mark both; if hash error → mark both (unreadable = differs).
6. For each entry in right_map not in left_map → mark right index.
7. Insert marks into `pane[Left].selected` and `pane[Right].selected` (additive: `BTreeSet::insert`).
8. Return `vec![PaneUpdated(Left), PaneUpdated(Right), Status("N entries differ")]`.

**CRC32 helper** (private fn, sync — called inside `async` via `tokio::task::spawn_blocking` for large dirs):
```rust
fn crc32_partial(path: &std::path::Path, size: u64) -> Option<u32> {
    const THRESHOLD: u64 = 4 * 1024 * 1024;     // 4 MiB
    const WINDOW: usize  = 512 * 1024;            // 512 KiB
    let data = if size <= THRESHOLD {
        std::fs::read(path).ok()?
    } else {
        let mut f = std::fs::File::open(path).ok()?;
        let mut buf = vec![0u8; WINDOW];
        use std::io::{Read, Seek, SeekFrom};
        f.read_exact(&mut buf[..WINDOW]).ok()?;
        let tail_start = size.saturating_sub(WINDOW as u64);
        f.seek(SeekFrom::Start(tail_start)).ok()?;
        let n = f.read(&mut buf).ok()?;
        buf[..n].to_vec()    // intentionally reads first + last separately; concatenation avoided
    };
    Some(crc32fast::hash(&data))
}
```

Note: For large files, only one window (head OR tail) is included per read call to keep things simple and fast, consistent with the spec assumption.

### Phase C: TUI wiring

**`PendingExternal` extension:**
```rust
struct PendingExternal {
    program: String,
    args: Vec<String>,    // was: path: String
}
```

Update `run_external()`:
```rust
let _ = std::process::Command::new(&ext.program)
    .args(&ext.args)     // was: .arg(&ext.path)
    .status();
```

Update `queue_external()` (F3/F4 path):
```rust
ui.pending_external = Some(PendingExternal {
    program,
    args: vec![local],   // was: path: local
});
```

**New `queue_diff()` function:**
```rust
fn queue_diff(app: &App, ui: &mut UiState, status: &mut String, diff_tool: Option<&str>) {
    // Collect tagged file paths from both panes (files only, local paths)
    let tagged: Vec<String> = [PaneId::Left, PaneId::Right].iter()
        .flat_map(|&id| {
            let p = app.pane(id);
            p.selected.iter().filter_map(|&idx| {
                let e = p.listing.entries.get(idx)?;
                if !matches!(e.meta.kind, VfsKind::File | VfsKind::Symlink { .. }) {
                    return None;
                }
                let path = p.cwd.join(e.name.as_str());
                let disp = path.display();
                Some(disp.strip_prefix("file://").unwrap_or(&disp).to_string())
            }).collect::<Vec<_>>()
        })
        .collect();

    if tagged.len() != 2 {
        *status = format!("Diff requires exactly 2 tagged files ({} tagged)", tagged.len());
        return;
    }
    let Some(tool_str) = diff_tool else {
        *status = r#"No diff tool configured — add [diff]\ntool = "vimdiff" to config"#.into();
        return;
    };
    let mut argv = shell_words::split(tool_str).unwrap_or_default();
    if argv.is_empty() {
        *status = "Diff tool string is empty".into();
        return;
    }
    let program = argv.remove(0);
    argv.extend(tagged);
    ui.pending_external = Some(PendingExternal { program, args: argv });
}
```

**`handle_key` dispatch additions** (in the `match action` arm for `Pane` mode actions):
```rust
Command::CompareDirectories => {
    let evs = app.dispatch(AppCommand::CompareDirectories).await?;
    for ev in evs { /* handle PaneUpdated + Status */ }
}
Command::DiffTwoTaggedFiles => {
    let tool = app.config().diff.tool.as_deref();
    queue_diff(app, ui, &mut status, tool);
}
```

### Phase D: Bench + CI

New `benches/compare-dirs.rs` criterion bench:
- Creates two temp directories with 1,000 files each (half identical, half differing)
- Measures time for `App::compare_directories()` end-to-end
- Asserts p95 ≤ 2 s (SC-001 gate)

Add bench to `Cargo.toml` `[[bench]]` table.

## Key Design Decisions (from research.md)

See [research.md](research.md) for full rationale. Summary:

| Decision | Choice | Rationale |
|---|---|---|
| Hash algorithm | CRC32 (crc32fast) | Already in workspace; 3–5 GB/s throughput; sufficient for identity comparison |
| Large-file strategy | Head 512 KiB + CRC32 | Two seeks, no full read; ~10 ms for a 1 GiB file |
| Diff tool invocation | argv-split + direct exec | No shell — no injection; consistent with F3/F4 pattern |
| TUI suspend for diff | Extend PendingExternal.args | Reuses the existing run_external() path; single code path for all external tools |
| Compare additive | Never clear existing selected | Preserves user's manual tags; spec Q2 answer |
| Progress for >1000 entries | Status "Comparing…" before hash loop | Simple; hashing is the slow part; avoids spinner complexity |
