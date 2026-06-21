# Research — Feature 061: Survivability, Crash Safety & About/Version Surface

Phase 0 decisions. Each item: **Decision / Rationale / Alternatives considered**.

---

## R1 — Panic strategy: switch release to `panic = "unwind"`

**Decision**: Change `[profile.release] panic = "abort"` → `"unwind"` in the root
`Cargo.toml`. Leave `opt-level = "z"`, `lto = "fat"`, `codegen-units = 1`,
`strip = "symbols"` unchanged.

**Rationale**: `std::panic::catch_unwind` is a **no-op under `abort`** — the
process dies before any catch runs. In-session recovery (US2, confirmed in
clarify) is impossible without unwinding. Unwinding also makes tokio isolate
task panics (a panicking task becomes a `JoinError`, the runtime survives) rather
than aborting the whole process (FR-008). The cost is unwinding tables in the
binary; current release binary ≈ 2.97 MiB against an 8 MiB ceiling (NFR-001),
leaving ample headroom. The exact delta is **measured during implementation** and
gated by `scripts/check-binary-size.sh` (SC-007); if it ever threatened the
ceiling, `panic="unwind"` would still win and other size levers exist, but that
is not expected.

**Alternatives considered**:
- *Keep `abort`, restore terminal + write report in the hook, then die.* Simpler
  and smallest, but cannot satisfy US2 (no recovery) — rejected per clarify.
- *`abort` + `libc::sigaction` SIGABRT handler to restore the terminal.* Fragile,
  `unsafe`, async-signal-safety landmines, still no recovery — rejected.

---

## R2 — Crash-handling architecture: capture-in-hook, decide-at-catch

**Decision**: The global panic hook performs only thread-safe capture: fill a
process-global `Mutex<Option<CapturedPanic>>` with message, location,
`Backtrace::force_capture()`, thread name, and a clone of the recent-action ring
buffer; emit `tracing::error!`. It does **not** restore the terminal and does
**not** write files. Outcome is decided by which `catch_unwind` catches the
unwinding panic:
- **Inner / recoverable** boundaries — the synchronous `term.draw(...)`, the
  async key/mouse handler (`FutureExt::catch_unwind` over `AssertUnwindSafe`),
  and each spawned transfer task — recover: read the captured panic, log + set a
  dismissible status message, continue. **No crash file.**
- **Outer / fatal** boundary — `catch_unwind` wrapping the whole UI session in
  `run()` and the startup body in `main` — is reached only when no inner boundary
  handled the fault: run the existing terminal teardown, then write one
  `crash-<ts>.log`, print the path, exit non-zero.

**Rationale**: Two hard problems vanish. (a) *Restoring the terminal from a
background-task panic would scramble the live UI* — solved by never touching the
terminal in the hook; the live UI keeps its raw mode, and only the outer fatal
boundary (already followed by teardown) restores it. (b) *Distinguishing
recovered vs fatal without a thread-local-across-`await` flag* — solved
structurally: recovered = caught by an inner boundary (no file); fatal = reached
the outer boundary (file). The hook is the *only* place a backtrace can be
captured (after `catch_unwind` returns, the stack is already unwound and
`Backtrace` would be empty), which is exactly why capture lives there.

**Alternatives considered**:
- *Hook writes the file + restores terminal, guarded by a thread-local "recovery
  depth".* The depth flag does not follow a tokio task across `await`/threads, so
  it misclassifies async-handler panics — rejected as fragile.
- *`human-panic` crate.* Pulls a dependency and an opinionated report format,
  still needs custom terminal-restore and gives no recovery — rejected
  (dependency-minimalism + insufficient).

---

## R3 — Backtrace capture without a dependency

**Decision**: Use `std::backtrace::Backtrace::force_capture()` inside the panic
hook (stable since Rust 1.65; workspace MSRV 1.76). `force_capture` ignores
`RUST_BACKTRACE`, satisfying FR-004 ("present regardless of env").

**Rationale**: Zero new dependencies; std-native; works under `unwind`. Symbol
quality depends on debug info — release strips symbols, so frames may be partial,
but the panic **message + `#[track_caller]` location** (from `PanicHookInfo`) are
always present and are the primary locator (SC-006). A short note ships in the
report when symbols are stripped.

**Alternatives considered**:
- `backtrace` crate for richer symbolication — extra dependency + size; rejected.
- `color-backtrace` / `better-panic` — TTY-oriented, dependency cost; rejected.

---

## R4 — Recent-action ring buffer (crash context)

**Decision**: A process-global, fixed-capacity `Mutex<VecDeque<ActionRecord>>`
(capacity 64) in `cargonaut-core::diag`. `App::dispatch` pushes one record per
command (the `Command` variant name + a coarse, **non-sensitive** detail, e.g.
pane id / cursor index — never paths' contents or credentials). The panic hook
snapshots it into the `CapturedPanic`.

**Rationale**: `debug.log` only keeps WARN+, so the normal-flow trail leading to
a crash is otherwise lost (FR-005). A bounded `VecDeque` is O(1) and tiny.
Recording the command *variant* (not raw user data) keeps it useful yet
secret-free (FR-015 / SC-008). 64 entries is plenty to see the lead-up without
bloating the report.

**Alternatives considered**:
- *Reuse `tracing` with an in-memory ring layer.* Heavier (custom `Layer`,
  formatting), and conflates logging levels — rejected for a purpose-built buffer.
- *Record full command arguments.* Risks leaking paths/credentials — rejected;
  record variant + coarse, reviewed detail only.

---

## R5 — Redaction / no-secrets guarantee (FR-015, SC-008)

**Decision**: The crash report is assembled only from fields that are
secret-free by construction: version, OS/arch, timestamp, panic message,
location, backtrace, and the action-variant trail. The known secret in the
process is an SFTP password inside `SftpCredentials::Password` — which is never
placed in a `Command`, never logged, and never recorded in the ring buffer. A
unit test (SC-008) configures a sentinel secret, forces a crash, and asserts the
sentinel does not appear in the report.

**Rationale**: Cheapest robust guarantee is *don't collect secrets in the first
place* rather than scrub afterward. The action trail's "coarse detail" rule (R4)
is the enforcement point.

**Alternatives considered**:
- *Post-hoc regex scrub of the assembled report.* Brittle (unknown secret shapes)
  — rejected in favor of not-collecting.

---

## R6 — Crash file: location, naming, retention, next-launch notice

**Decision**: Write `crash-<UTC-timestamp>.log` to the existing data dir
(`$XDG_DATA_HOME/cargonaut` or `~/.local/share/cargonaut`, same as `debug.log`).
Timestamp format `YYYYMMDD-HHMMSSsss` for lexicographic = chronological sort.
Retention: after writing, keep the newest **10** `crash-*.log`, delete older
(FR-014). Next-launch notice (FR-006a / SC-009): a `crash-seen` marker file
stores the name (or mtime) of the most recent report the user was already told
about; on startup, if a newer `crash-*.log` exists, surface a one-time status
notice and update the marker.

**Rationale**: Co-locating with `debug.log` means one place to look and reuses
existing dir-creation. Lexical timestamp avoids a sort dependency. A simple
marker file is the minimal way to make the notice fire exactly once (SC-009).
Writing is **failure-tolerant** (FR-013): all IO returns `Result`; failure logs
and degrades to "could not save report" without a secondary panic, and the
terminal is still restored.

**Alternatives considered**:
- *Single rolling `crash.log`.* Loses history of distinct crashes — rejected.
- *Time-based retention (e.g. 7 days).* Needs wall-clock math; count-based is
  simpler and equally bounded — chosen count-based (10).

---

## R7 — Recoverable boundaries: which, and `UnwindSafe`

**Decision**: Three inner boundaries.
1. **Render**: wrap the synchronous `term.draw(|f| …)` in
   `std::panic::catch_unwind(AssertUnwindSafe(|| term.draw(…)))`. Cleanest, fully
   synchronous, no `await`. Most likely panic source (widget arithmetic, slicing).
2. **Input handling**: wrap the async key/mouse dispatch with
   `futures::FutureExt::catch_unwind` over `AssertUnwindSafe(fut)`.
3. **Background transfer task**: wrap the spawned task body so a panic resolves
   the job to `Failed` instead of vanishing.
On catch: read `CapturedPanic`, `tracing::error!`, set the status line to a short
"recovered from internal error — see <data-dir>/debug.log", continue.

**Rationale**: These three cover the spec's named recovery surfaces (drawing,
single input, background task). `AssertUnwindSafe` is required because `&mut App`
is not `UnwindSafe`; this is sound here because after a caught panic we return to
a known loop head and the next frame fully re-renders from `app` state. The
honest caveat — *recovered app state may be partially mutated* — is documented;
the contract is "stay alive and usable," not "transactional rollback." A repeated
fast-failing render is rate-limited (don't spin): after N consecutive recovered
render panics, escalate to fatal (write report + exit) to avoid a hot loop.

**Alternatives considered**:
- *Per-widget catch.* Too granular, scattered; rejected for one render-level catch.
- *No async-handler catch (input panics fatal).* Violates the clarified scope
  ("while handling a single input") — rejected.

---

## R8 — Test-only panic injection

**Decision**: Honor an env var `CARGONAUT_PANIC_INJECT=<site>` read at the
relevant points (`startup`, `render`, `input`, `task`). When set to a matching
site, code `panic!("injected: <site>")` once. No hidden CLI flag (env keeps clap
output clean and is trivial for the PTY harness to set). It is inert in normal
use (nobody sets it).

**Rationale**: Deterministic triggering of each fault class is required to gate
SC-001..SC-004 in CI. An env var is the least-surface, harness-friendly trigger
and mirrors the existing `CARGONAUT_PTY_TESTS` / `CARGONAUT_EXIT_CWD_FILE`
conventions.

**Alternatives considered**:
- *`#[cfg(test)]`-only hooks.* Don't exist in the release binary the PTY test
  spawns — rejected (PTY test runs the real binary).
- *Hidden clap flag.* Adds CLI surface and risks discovery; env is cleaner.

---

## R9 — About surface (in-app ×2 + CLI)

**Decision**: (a) Enrich `HELP_SECTIONS` "About" with lines built from
`env!("CARGO_PKG_VERSION")`, author/copyright, and `MIT OR Apache-2.0` (const
`&'static str`s via `concat!`). (b) Add `Command::ShowAbout` + an
`ActiveDialog::About` modal and a menu entry in `chrome.rs`. (c) clap:
`#[command(version, long_version = LONG_VERSION)]` where `LONG_VERSION` is a
`concat!` const of version + copyright + license; bare `--version` keeps the short
string, `--version` long output (and `-V`/`--help` footer) carries copyright.

**Rationale**: Reuses existing surfaces (help section, menu→command, clap derive)
with zero new dependencies. `AboutInfo` is a pure struct in `core::diag` so the
exact same strings feed all three surfaces and one unit test covers them
(SC-005). Author/copyright/license are sourced from the existing SPDX file
headers ("© 2024–2026 Mohiuddin Khan Inamdar", "MIT OR Apache-2.0").

**Alternatives considered**:
- *Only enrich help (no dialog).* Clarify chose "Both" — rejected.
- *Read authors from `CARGO_PKG_AUTHORS`.* Workspace may not populate it
  consistently; hardcode the reviewed copyright string in `AboutInfo` and unit-
  test it — chosen for determinism.

---

## Summary of decisions

| # | Area | Decision |
|---|------|----------|
| R1 | Panic strategy | release `panic = "unwind"` |
| R2 | Architecture | capture-in-hook, decide-at-catch (inner=recover, outer=fatal) |
| R3 | Backtrace | `std::backtrace::Backtrace::force_capture()`, no dep |
| R4 | Context | global 64-entry action ring buffer in `core::diag` |
| R5 | Secrets | don't-collect (variant-only trail); SC-008 sentinel test |
| R6 | Crash file | `crash-<ts>.log` in data dir, keep newest 10, `crash-seen` marker |
| R7 | Recovery | catch render (sync), input (async), transfer task; rate-limit |
| R8 | Test hook | `CARGONAUT_PANIC_INJECT={startup,render,input,task}` env |
| R9 | About | help section + About dialog + clap long_version; pure `AboutInfo` |

No `NEEDS CLARIFICATION` remain.
