# Contract — Crash report file format, naming & retention

## File naming

- Report: `crash-<TS>.log` where `<TS>` = UTC `YYYYMMDD-HHMMSSsss`
  (lexicographic order == chronological order; no sort dependency).
- Seen-marker: `crash-seen` (single file; content = name of last-acknowledged
  report).
- Location: the data dir — `$XDG_DATA_HOME/cargonaut` else
  `~/.local/share/cargonaut` (same dir as `debug.log`).

## File content (exact output of `format_crash_report`)

Stable, greppable section headings. Example:

```text
cargonaut crash report
=======================
version:   0.1.0
when:      20260621-143022417 UTC
platform:  linux/x86_64
thread:    main

## Panic
message:   index out of bounds: the len is 3 but the index is 7
location:  crates/cargonaut-ui-tui/src/pane.rs:412:18

## Recent actions (oldest first)
  1. FocusSwap
  2. CursorDown        pane=Left idx=11
  3. CursorDown        pane=Left idx=12
  4. Descend           pane=Left
  5. CycleListingMode

## Backtrace
<std::backtrace::Backtrace::force_capture() output>
(note: release builds strip symbols; frames may be partial — message + location above are authoritative)
```

## Guarantees

- **Always present**: `version`, `platform`, `## Panic` with `message`, `##
  Backtrace`. `location` present whenever the runtime provided one (SC-002,
  SC-006).
- **Never present**: credentials or raw file contents (FR-015 / SC-008) — the
  document is assembled only from secret-free inputs; the action trail carries
  variant labels + coarse reviewed detail only.
- **Deterministic**: given the same `ReportMeta` + `CapturedPanic`, byte-identical
  output (enables a stable unit test).

## Retention (FR-014)

- After each `write_report`, `prune_reports(dir, 10)` keeps the 10 newest
  `crash-*.log` (by lexical name) and deletes older. Default `keep = 10`.

## Next-launch notice (FR-006a / SC-009)

- On startup, `unseen_report(dir)` returns the newest `crash-*.log` whose name is
  lexically greater than the name stored in `crash-seen` (or any report if no
  marker). If `Some`, the app surfaces a one-time status notice with the path and
  calls `mark_seen`. Subsequent launches return `None` → no repeat (SC-009).

## Failure tolerance (FR-013)

- All IO returns `io::Result`. On any error (dir not writable, disk full):
  - the terminal is still restored,
  - the user is told "could not save crash report (<reason>)" instead of the path,
  - no secondary panic occurs.
