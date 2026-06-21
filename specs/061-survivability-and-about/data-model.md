# Data Model — Feature 061: Survivability, Crash Safety & About/Version Surface

All types live in **`cargonaut-core::diag`** unless noted. They are pure data +
pure functions where possible; IO is isolated and dir-injectable for tests.

---

## ActionRecord

One entry in the recent-action trail.

| Field | Type | Notes |
|-------|------|-------|
| `seq` | `u64` | monotonically increasing sequence number |
| `label` | `String` | command/event variant name, e.g. `"CursorDown"`, `"Copy"` |
| `detail` | `Option<String>` | coarse, **secret-free** context (e.g. `"pane=Left idx=12"`); never raw paths/credentials |

**Validation / rules**: `label` is a stable identifier (the `Command` variant
name). `detail` MUST NOT contain user-supplied free text, file contents, or
credentials (FR-015). Constructed only by `record_action`.

---

## RecentActionBuffer

Process-global, fixed-capacity ring of `ActionRecord`.

- Backed by a `Mutex<VecDeque<ActionRecord>>` in a `OnceLock`/`static`.
- **Capacity**: 64 (oldest dropped on overflow).
- API: `record_action(label, detail)`; `snapshot() -> Vec<ActionRecord>` (newest
  last); `clear()` (tests).

**State transitions**: push → (if len > cap) pop_front. No other states.

**Relationships**: snapshotted into `CapturedPanic` by the panic hook.

---

## CapturedPanic

What the panic hook records at fault time. Process-global
`Mutex<Option<CapturedPanic>>`.

| Field | Type | Source |
|-------|------|--------|
| `message` | `String` | `PanicHookInfo::payload()` downcast to `&str`/`String` |
| `location` | `Option<String>` | `PanicHookInfo::location()` → `file:line:col` |
| `thread` | `String` | `std::thread::current().name()` or `"unnamed"` |
| `backtrace` | `String` | `Backtrace::force_capture().to_string()` |
| `actions` | `Vec<ActionRecord>` | `RecentActionBuffer::snapshot()` |

**Rules**: filled by the hook; *taken* (`Option::take`) by a catch site. A
recoverable catch site takes it, logs, and discards. The fatal catch site takes
it and formats a `CrashReport`. Backtrace MUST be captured here (post-unwind it is
empty).

---

## CrashReport (formatted document)

Pure function `format_crash_report(meta: &ReportMeta, panic: &CapturedPanic) ->
String`. No IO. The on-disk file content is exactly this string.

`ReportMeta` (injected, deterministic for tests):

| Field | Type | Source |
|-------|------|--------|
| `app_version` | `&'static str` | `env!("CARGO_PKG_VERSION")` |
| `os` | `&'static str` | `std::env::consts::OS` |
| `arch` | `&'static str` | `std::env::consts::ARCH` |
| `timestamp` | `String` | injected (UTC `YYYYMMDD-HHMMSSsss`) |

**Document sections** (see `contracts/crash-report-format.md`): header
(app+version+timestamp+platform), `## Panic` (message, location, thread),
`## Recent actions` (oldest→newest), `## Backtrace`. Stable, greppable headings.

**Rules**: contains none of: credentials, raw file contents (FR-015/SC-008).
Always includes version, platform, location (if any), backtrace (SC-002/SC-006).

---

## CrashLog (IO + lifecycle)

Filesystem operations, dir-injectable (`fn …(dir: &Path, …)`), all return
`io::Result` and never panic (FR-013).

- `write_report(dir, timestamp, body) -> io::Result<PathBuf>` — write
  `crash-<timestamp>.log`.
- `prune_reports(dir, keep: usize) -> io::Result<()>` — keep newest `keep`
  (default 10) `crash-*.log`, delete older (FR-014).
- `unseen_report(dir) -> io::Result<Option<PathBuf>>` — newest `crash-*.log`
  newer than the `crash-seen` marker, else `None` (FR-006a).
- `mark_seen(dir, report: &Path) -> io::Result<()>` — record the report as
  acknowledged (write its name to `crash-seen`).

**Naming**: `crash-YYYYMMDD-HHMMSSsss.log`; marker `crash-seen`.

**State**: report file is write-once; `crash-seen` is overwritten on each
acknowledgement. Retention is enforced after every `write_report`.

---

## AboutInfo

Static identity, pure data; single source for all three About surfaces.

| Field | Type | Value/source |
|-------|------|--------------|
| `name` | `&'static str` | `"cargonaut"` |
| `version` | `&'static str` | `env!("CARGO_PKG_VERSION")` |
| `author` | `&'static str` | `"Mohiuddin Khan Inamdar"` |
| `copyright` | `&'static str` | `"© 2024–2026 Mohiuddin Khan Inamdar"` |
| `license` | `&'static str` | `"MIT OR Apache-2.0"` |
| `repository` | `&'static str` | project URL |

- `fn about_lines() -> Vec<String>` — the rendered lines shared by the F1 Help
  "About" section, the About dialog, and (joined) the clap `long_version`.

---

## Command (extension to existing enum, `cargonaut-core::command`)

| Variant | Meaning |
|---------|---------|
| `ShowAbout` | open the dedicated About view (`ActiveDialog::About`) |

Recorded in the ring buffer like any other command.

---

## ActiveDialog::About (UI, `cargonaut-ui-tui`)

New modal variant rendering `AboutInfo::about_lines()`. Dismissed by Esc/Enter.
No state beyond a scroll offset (content is short; offset optional).

---

## Relationships

```text
App::dispatch ──record_action──▶ RecentActionBuffer
                                       │ snapshot()
panic! ──hook──▶ CapturedPanic ◀───────┘
   │ taken by
   ├─ inner catch (recover): log + status, discard            (no file)
   └─ outer catch (fatal): format_crash_report(meta, panic)
                                  │
                                  ▼
                           CrashLog.write_report → prune_reports
                                  │
                     next launch: unseen_report → notice → mark_seen

AboutInfo ──about_lines()──▶ { HELP_SECTIONS "About", ActiveDialog::About, clap long_version }
```
