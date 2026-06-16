# Implementation Plan: File Attribute Operations (chmod / chown / links)

**Branch**: `043-file-attributes` | **Date**: 2026-06-17 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/043-file-attributes/spec.md`

## Summary

Add the reference orthodox-FM file-attribute operations (issue #46): change Unix
permissions (chmod, octal + symbolic), change ownership (chown, user/group), and
create symbolic + hard links — over the local filesystem. New `VfsBackend`
operations (`chmod`/`chown`/`symlink`/`hard_link`) are implemented for `LocalFs`
(via `std::os::unix::fs` + `nix` for name→uid/gid), defaulting to
`VfsError::Unsupported` on other backends (FR-006). The operations apply to the
current selection (tagged files, else focused entry, never the `..` row),
surface through the existing shared dialog widgets, and are reachable from the
File menu plus the orthodox keymap chords `C-x c/o/s/l`.

Mode parsing is a **pure, testable** `ModeSpec` in `cargonaut-vfs` — octal
(`644`/`0755`) or symbolic (`u+x`, `go-w`, `a=r`), applied per-file to each
file's current bits (so a symbolic change is relative; octal is absolute). This
is the SC-004 invalid-input gate.

Recursion (whole-subtree apply) is **out of scope** (clarified) and tracked as a
follow-up before merge.

## Technical Context

**Language/Version**: Rust (workspace edition; same toolchain as the rest).

**Primary Dependencies**: existing `cargonaut-vfs` (`VfsBackend`, `LocalFs`,
`VfsMetadata`/`FileMode` which already carry `bits`/`uid`/`gid`, `map_io`),
`cargonaut-core` (`App`, `selection_or_focused`, `refresh_active_pane`),
`cargonaut-ui-tui` (`TextInputDialog`, `ConfirmDialog`, menu, dispatch). Standard
library `std::os::unix::fs` (`chown`, `symlink`, `PermissionsExt`) +
`std::fs::{set_permissions, hard_link}`. **`nix`** (already in the lockfile as a
transitive dep) promoted to a direct dep of `cargonaut-vfs` for **safe**
`User::from_name`/`Group::from_name` name resolution — no `unsafe`.

**Storage**: N/A — operations mutate the filesystem directly; no persisted state.

**Testing**: `cargo test --workspace`. `ModeSpec` parse/apply truth table
(pure, SC-004) in `cargonaut-vfs`. VFS op tests (chmod/chown/symlink/hard_link)
against `tempfile` dirs in `cargonaut-vfs` (chown asserted via re-stat; the
numeric-noop / chgrp-to-own-gid case keeps it runnable unprivileged). Core
selection/op tests (`#[tokio::test]`, tempdirs) for chmod_selection /
create_symlink etc. incl. partial-failure reporting. UI dialog + dispatch tests
(synthetic keys, `TestBackend`) mirroring Features 038/042.

**Target Platform**: Linux terminal, local filesystem (Unix permission model).

**Project Type**: Single Rust workspace (multi-crate). Touches vfs (ops +
parsing), core (orchestration), ui-tui (dialogs/menu/keymap) — one concern each.

**Performance Goals**: Honor NFR-002 (≤16 ms keypress→first-paint). Each op is a
handful of syscalls on the selection, run in the async dispatch path, off the
render loop. No new per-frame cost.

**Constraints**: `unsafe`-free (`nix` provides safe name lookup).
`#![warn(missing_docs)]` on new public items. New bindings land in
`design/contracts/keymap.toml` first (Constitution III). NFR-001 (≤8 MiB) — `nix`
is already in the dependency tree, so negligible binary impact.

**Scale/Scope**: 4 new `VfsBackend` methods + `LocalFs` impls + a `mode` module
in vfs; `AppError::BadAttr` + 4 App methods + dispatch in core; 4 `Command`
variants + keymap bindings + File-menu entries + dialog/InputKind wiring + help
in ui-tui; plus tests. Selection model, refresh, and dialog widgets are all
reused.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Code Quality**: No `unsafe` (name resolution via safe `nix`; ops via std).
  New public items (`VfsBackend::chmod/chown/symlink/hard_link`, `ModeSpec`,
  `App` methods, `Command` variants) carry doc comments. clippy `-D warnings` +
  `cargo fmt --check` enforced. ✅ (no waivers)
- **II. Test-First**: Every FR/SC gets a failing test first then green, with
  `(red)`→`(green)` history. Pure `ModeSpec` parse/apply is the SC-004 gate; VFS
  op tests gate SC-001/002/006; core tests gate SC-003/005 (multi-file +
  partial-failure); UI tests gate SC-007 (menu/keys/cancel). ✅
- **III. UX Consistency**: Dialogs reuse `TextInputDialog`/`ConfirmDialog` (no
  ad-hoc layouts). Bindings (`C-x c/o/s/l`) added to `keymap.toml` first; File
  menu updated. Help overlay documents the new keys. Typed theme only. ✅
- **IV. Performance (NFR-001/002)**: `nix` already in-tree → no binary growth.
  Ops are off the render path. ✅
- **V. SSD Preservation**: dev build/test via `make`; no `cargo clean`/`rm -rf
  target`. ✅

**Result**: PASS — no violations, Complexity Tracking empty.

## Project Structure

### Documentation (this feature)

```text
specs/043-file-attributes/
├── plan.md · research.md · data-model.md · quickstart.md
├── contracts/attr-ops-seam.md
└── tasks.md   (/speckit-tasks — not created here)
```

### Source Code (repository root)

```text
crates/cargonaut-vfs/src/
├── traits.rs   # + VfsBackend::chmod/chown/symlink/hard_link (default Unsupported)
├── local.rs    # + LocalFs impls (std::os::unix::fs + nix name lookup) + tests
├── mode.rs     # + ModeSpec (octal/symbolic parse + apply) + tests   [new]
└── lib.rs      # + pub mod mode; re-exports
crates/cargonaut-vfs/Cargo.toml   # + nix (already transitive) as direct dep

crates/cargonaut-core/src/
└── lib.rs      # + AppError::BadAttr; chmod_selection/chown_selection/
                #   create_symlink/create_hard_link; dispatch + tests

crates/cargonaut-ui-tui/src/
├── keymap.rs   # + Command::{Chmod,Chown,CreateSymlink,CreateHardLink}
└── lib.rs      # + dispatch arms open dialogs; InputKind variants; chown confirm
                #   chain; File-menu entries; help text + tests
design/contracts/keymap.toml      # + C-x c/o/s/l bindings (pane mode)
```

**Structure Decision**: Same three-crate split by concern used by Feature 042
(vfs = ops + pure parsing, core = orchestration over the selection, ui-tui =
dialogs/menu/keymap). chmod/chown/links flow through the established
"input dialog → App method → refresh" pattern (`App::mkdir` is the template);
chown additionally chains a `ConfirmDialog` (FR-007).

## Complexity Tracking

> No constitution violations — section intentionally empty.
