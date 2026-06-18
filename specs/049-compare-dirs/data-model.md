# Data Model: Compare Directories + Diff Tagged Files (Feature 049)

**Date**: 2026-06-18

## Existing entities (unchanged)

### `PaneState` (`cargonaut-core`)
| Field | Type | Role in this feature |
|---|---|---|
| `cwd` | `VfsPath` | Base path for resolving absolute file paths during hash |
| `listing` | `DirListing` | Source of entries for comparison |
| `selected` | `BTreeSet<usize>` | Receives compare-marks (additive insert only) |
| `visible_indices()` | `Vec<usize>` | Determines which entries participate in compare |

### `DirEntry` (`cargonaut-vfs`)
| Field | Type | Role in this feature |
|---|---|---|
| `name` | `SmolStr` | Comparison key (matched by exact name across panels) |
| `meta.size` | `u64` | First comparison tier (fast size check before hash) |
| `meta.kind` | `VfsKind` | Guards against hashing dirs or unsupported types |

### `VfsKind` (relevant values)
| Variant | Compare behaviour |
|---|---|
| `File` | Name + size + CRC32 hash |
| `Dir` | Name-presence only (never hashed, never marked for content diff) |
| `Symlink { .. }` | Treated as File: compare size of target (stat, not lstat) |
| `Other` | Name-presence only (treated like Dir) |

## New entities

### `DiffConfig` (`cargonaut-config`)
Added to the `Config` root struct as `pub diff: DiffConfig`.

| Field | Type | Default | Constraint |
|---|---|---|---|
| `tool` | `Option<String>` | `None` | Argv string; split on whitespace; first token is the binary |

**State transitions**: N/A — stateless config value read at action time.

**Validation rules**:
- `None` → "Diff tagged files" action shows error, no process launched.
- `Some("")` → treated as empty; shows error "Diff tool string is empty".
- `Some("vimdiff -O2")` → splits to `["vimdiff", "-O2"]`; `path1`, `path2` appended.

### `PendingExternal` (`cargonaut-ui-tui`, modified)
| Field | Before | After |
|---|---|---|
| `program` | `String` | `String` (unchanged) |
| ~~`path`~~ | `String` | removed |
| `args` | — | `Vec<String>` (new) |

**Behaviour**: `run_external()` passes `args` as positional arguments: `.args(&ext.args)`.

F3/F4 migration: `path: local` → `args: vec![local]` (one-element slice; no behavioural change).

## Compare result (transient, ephemeral)

The compare result is not persisted or stored as a named type. It manifests solely as entries inserted into `PaneState.selected`. The classification is computed inline and discarded:

| Classification | Action |
|---|---|
| `left-only` (name absent in right) | Insert left-pane index into `pane[Left].selected` |
| `right-only` (name absent in left) | Insert right-pane index into `pane[Right].selected` |
| `size-differ` (same name, different size) | Insert both indices |
| `hash-differ` (same name+size, different CRC32) | Insert both indices |
| `unreadable` (I/O error during hash) | Insert both indices (treat as differing) |
| `identical` (same name+size+hash) | No insert |

Subdirectories: never hashed; if present on both sides → `identical` regardless of contents.

## Key invariants

1. `compare_directories()` is strictly additive: it only calls `BTreeSet::insert`, never `remove` or `clear`.
2. Indices in `selected` refer to positions in `listing.entries` at the time of compare. A listing reload (e.g., from disk change) may invalidate these indices — the user must re-run compare if they reload.
3. The diff tool receives exactly two path strings as the last two positional arguments, in the order: left-tagged-file, right-tagged-file (by pane order, not by tag insertion order).
