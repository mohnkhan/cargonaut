# Contract: `menu.toml` Schema

**Feature**: 047-user-menu-help
**Date**: 2026-06-18

---

## File Location

```
$XDG_CONFIG_HOME/cargonaut/menu.toml
  └── fallback: $HOME/.config/cargonaut/menu.toml
```

## Format

The file is TOML. The top-level table contains an `actions` array of action tables.

```toml
# Each [[actions]] entry defines one item in the F2 user menu.

[[actions]]
# REQUIRED. The display label shown in the F2 menu.
label = "Open in $EDITOR"

# REQUIRED. The shell command to run. Supports one placeholder:
#   {path} — replaced by the shell-quoted absolute path of the highlighted entry.
# If the command contains shell operators (|  ;  &&  ||  $  `  >  <),
# the application runs it as: sh -c "<command>"
# Otherwise it is split into tokens and run directly (no shell involved).
command = "$EDITOR {path}"

# OPTIONAL. A shell expression evaluated at menu-open time.
# The action is shown only when the expression exits 0 (true).
# Evaluated with a 200 ms timeout; a timeout = hidden.
# Omit (or leave empty) to always show the action.
only_if = "test -f {path}"

# OPTIONAL. A single printable ASCII character.
# Pressing this key while the F2 menu is open executes the action immediately.
# If two actions share the same key, the first one wins.
key = "e"
```

## Constraints

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `label` | string | YES | Non-empty; truncated at 60 chars for display |
| `command` | string | YES | Non-empty; `{path}` is the only recognized placeholder |
| `only_if` | string | no | Shell expression; empty = always visible |
| `key` | string | no | Exactly one printable ASCII character |

## Placeholder Substitution

The `{path}` placeholder in `command` and `only_if` is replaced with the absolute path of the highlighted entry, **shell-quoted** using POSIX single-quoting so that paths with spaces, quotes, and backslashes are handled safely.

Example — if the highlighted entry is `/home/user/my file.txt`:

| Template | Expanded |
|----------|----------|
| `cat {path}` | `cat '/home/user/my file.txt'` |
| `wc -l {path}` | `wc -l '/home/user/my file.txt'` |
| `test -f {path}` | `test -f '/home/user/my file.txt'` |

If `{path}` does not appear in `command`, the command runs as-is (useful for actions that do not need a file argument).

## Error Behavior

| Situation | Behavior |
|-----------|----------|
| File missing | F2 menu shows placeholder row; no error |
| File empty / no `[[actions]]` | F2 menu shows placeholder row; no error |
| TOML parse error | F2 menu shows error message (filename + line number); app continues |
| `only_if` times out | Action is hidden for this open |
| Command exits non-zero | Status bar shows `[exit N] <first stderr line>` |
| Command binary not found | Status bar shows `[exit 127] command not found: <prog>` |

## Complete Example

```toml
[[actions]]
label   = "Edit"
command = "$EDITOR {path}"
key     = "e"

[[actions]]
label   = "Open terminal here"
command = "xterm -e bash"
only_if = "test -d {path}"
key     = "t"

[[actions]]
label   = "Show git log"
command = "git -C {path} log --oneline -20 | less"
only_if = "git -C {path} rev-parse --is-inside-work-tree 2>/dev/null"
key     = "g"

[[actions]]
label   = "Compress (tar.gz)"
command = "tar czf {path}.tar.gz {path}"
key     = "z"

[[actions]]
label   = "Copy path to clipboard"
command = "printf '%s' {path} | xclip -selection clipboard"
```
