# Contract: Commands Delta (keymap → core wiring)

The set of keymap `Command`s this feature lights up (today they return `None` in `ui_command_to_core`, lib.rs:288-316, or are absent from core). Keymap remains the single source of truth (constitution §III) — bindings land in `design/contracts/keymap.toml` first.

## Newly wired (US2/US4/US5)

| keymap::Command | key (canonical) | core AppCommand | FR |
|-----------------|-----------------|-----------------|----|
| `Mkdir` | F7 | `Mkdir(name)` (via prompt dialog) | FR-024 |
| `SelectionAddByPattern` | `+` | `SelectByPattern(glob)` | FR-025 |
| `SelectionRemoveByPattern` | `-` | `UnselectByPattern(glob)` | FR-025 |
| `CycleSortKey` | C-s | `CycleSortKey` (+ reverse toggle) | FR-021 |
| `CycleListingMode` | M-t | `CycleListingMode` (brief/full/quick-view) | FR-022 |
| `RecursiveDirSize` | C-Space | `RecursiveDirSize` | FR-023 |
| `Preview` (F3) | F3 | `ViewExternal` (`$PAGER`) | FR-030 |
| `Edit` (F4) | F4 | `EditExternal` (`$EDITOR`) | FR-031 |
| `OpenMenuBar` (F9) | F9 | `OpenMenuBar` | FR-009 |
| `ShowHelp` (F1) | F1 | `ShowHelp` (minimal help overlay or "not yet available") | FR-008/011 |

## New internal command (US3)

| AppCommand | origin | FR |
|-----------|--------|----|
| `CursorTo(usize)` | mouse click row → set absolute cursor (survives `sync_from`) | FR-014 |

## Still deferred (label shown on bar, action reports "not yet available" — FR-011)

`ShowUserMenu` (F2), `OpenSubshell` (C-o), `BookmarksMenu` (C-b), `NewTab`/`CloseTab`, `ExternalPanelize`, `CompareDirectories`, `DiffTwoTaggedFiles`, `BulkRenameViaEditor`, `UndoLastOp`, find-file (not in keymap yet). Each MUST have a tracked deferral (issue + ROADMAP row) per FR-029.

## Invocation parity (FR-028)

Every newly wired command MUST be invokable by its existing key binding AND by its menu/F-key-bar affordance, mapping to the identical core `AppCommand`. No existing binding changes behavior.

## Invariants (testable)

- T-CMD-1: each row in "Newly wired" returns `Some(core_cmd)` from `ui_command_to_core` (no longer `None`).
- T-CMD-2: pressing F7 and clicking fkey button #7 both reach `Mkdir`.
- T-CMD-3: a deferred command surfaces a "not yet available" status, never a silent no-op (SC-005).
