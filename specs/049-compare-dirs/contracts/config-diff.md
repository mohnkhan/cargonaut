# Contract: `[diff]` Config Section (Feature 049)

**Source of truth**: `crates/cargonaut-config/src/lib.rs` (`DiffConfig` struct)  
**Schema**: `design/contracts/config.schema.json` (regenerated after implementation)

## TOML shape

```toml
[diff]
# Argv string for the external diff tool.
# Split on whitespace; the two tagged file paths are appended as final args.
# Examples:
#   tool = "vimdiff"
#   tool = "diff -u"
#   tool = "meld --diff"
tool = "vimdiff"
```

## Rust type

```rust
pub struct DiffConfig {
    pub tool: Option<String>,
}
```

## Semantics

| Config value | Behaviour |
|---|---|
| absent / `None` | "Diff tagged files" is disabled; action shows error |
| `""` (empty string) | Error: "Diff tool string is empty" |
| `"vimdiff"` | Invokes `vimdiff <path1> <path2>` via direct exec |
| `"diff -u"` | Invokes `diff -u <path1> <path2>` via direct exec |

## Exec contract

The tool is invoked as:
```
argv[0]  argv[1..]  path1  path2
```

Where:
- `argv` = `shell_words::split(tool)` — whitespace-split, shell-quote-aware.
- `path1` = absolute local path of the first tagged file (left-pane order first).
- `path2` = absolute local path of the second tagged file.
- No shell is invoked. The binary must be on `$PATH`.

## User documentation note

For GUI tools that do not need the terminal: the TUI still suspends and waits for the tool process to exit. Users who want the TUI to remain active while a GUI tool is open should wrap the tool in a background-launcher script (e.g., `meld-bg` that does `meld "$@" & disown`).
