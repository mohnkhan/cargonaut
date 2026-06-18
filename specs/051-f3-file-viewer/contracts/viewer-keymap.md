# Viewer Keymap Contract

**Feature**: 051-f3-file-viewer | **Date**: 2026-06-19

This document specifies the exact key bindings for the built-in file viewer (F3). All bindings under `mode = "preview"` in `design/contracts/keymap.toml` are authoritative; this document provides the rationale and format contract.

---

## Existing preview-mode bindings (FR-209 — unchanged by this feature)

| Key | TOML action | Command enum | Behaviour |
|-----|-------------|--------------|-----------|
| `Ctrl-x X` | `toggle-hex-view` | `ToggleHexView` | Toggle text/hex mode; scroll resets; search cleared |
| `/` | `preview-search-forward` | `PreviewSearchForward` | Open forward search prompt |
| `?` | `preview-search-backward` | `PreviewSearchBackward` | Open backward search prompt |
| `n` | `preview-search-next` | `PreviewSearchNext` | Advance to next match |
| `N` | `preview-search-prev` | `PreviewSearchPrev` | Go to previous match |

---

## New preview-mode bindings (this feature — FR-031)

| Key | TOML action | Command enum | Behaviour |
|-----|-------------|--------------|-----------|
| `g` | `viewer-goto` | `ViewerGoto` | Open goto prompt (line # in text, byte offset in hex) |
| `G` | `viewer-end` | `ViewerEnd` | Jump to last line (text) or last 16-byte row (hex) |
| `w` | `viewer-wrap` | `ViewerWrap` | Toggle word-wrap (text mode only; no-op in hex) |
| `q` | `viewer-quit` | `ViewerQuit` | Close the viewer |

---

## Standard navigation (handled directly in `FileViewerDialog::handle_key`, not via Command enum)

| Key | Behaviour |
|-----|-----------|
| `Up` | Scroll up 1 line / 1 hex row |
| `Down` | Scroll down 1 line / 1 hex row |
| `Page Up` | Scroll up by ~viewport height |
| `Page Down` | Scroll down by ~viewport height |
| `Home` | Jump to line 1 / byte offset 0 |
| `End` | Jump to last line / last hex row (same as `G`) |
| `Esc` | Close the viewer (when no prompt active); close prompt (when prompt active) |

---

## Status line format contract

**Text mode (normal)**:
```
Line <N>/<TOTAL>  [<filename>]  [wrap: on/off]
```
Example: `Line 42/3512  config.toml  wrap: off`

**Hex mode (normal)**:
```
Offset 0x<HEX_OFFSET> / <TOTAL_BYTES> bytes  [<filename>]
```
Example: `Offset 0x000A0000 / 524288 bytes  binary.dat`

**Search — match found (loaded file)**:
```
/<pattern>  match <N>  Line <L>
```
Example: `/error  match 3  Line 128`

**Search — match found (streaming file — FR-033)**:
```
/<pattern>  match <N>  Line <L>  (searched <X> MiB of <Y> MiB)
```
Example: `/error  match 1  Line 842  (searched 10 MiB of 512 MiB)`

**Search — no match (loaded file)**:
```
Pattern not found: <pattern>
```

**Search — no match (streaming file — FR-033)**:
```
Pattern not found: <pattern>  (searched <X> MiB of <Y> MiB)
```

---

## Hex row format contract (FR-013)

Each row is exactly this format (cols are space-separated):

```
<8-digit-hex-offset>  <8 × "HH ">  <8 × "HH ">  |<16 ASCII chars>|
```

Example:
```
00000000  48 65 6c 6c 6f 2c 20 57  6f 72 6c 64 21 0a 00 00  |Hello, World!...|
```

- Offset: 8 hex digits, zero-padded, lowercase
- Hex bytes: two lowercase hex digits, space-separated, split into two groups of 8 with an extra space between groups
- ASCII column: printable ASCII chars as-is; bytes outside 0x20–0x7E displayed as `.`
- Borders: `|` on each side of the ASCII column

---

## Goto prompt format contract

**Text mode**:
```
Go to line: _
```
Accepts: decimal integer (e.g., `1200`). Out-of-range values are clamped to [1, last_line].

**Hex mode**:
```
Go to offset: _
```
Accepts: decimal integer (e.g., `4096`) or `0x`-prefixed hexadecimal (e.g., `0x1000`). Out-of-range values are clamped to [0, last_row_start].

---

## Title bar format contract

```
 F3 View — <filename>  [<mode>] 
```

Example: ` F3 View — config.toml  [text] ` or ` F3 View — binary.dat  [hex] `

- `<mode>`: `text` or `hex`
- Title shown in the overlay border (ratatui `Block::title`)
