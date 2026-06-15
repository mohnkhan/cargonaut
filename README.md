# Cargonaut

> **The orthodox dual-pane file manager, rebuilt in Rust for 2026.**
> Keyboard-first. Mouse-friendly. Crash-safe transfers. ~2 MiB, no runtime.

Cargonaut brings back the fastest way ever invented to move files around a
machine — two panes, source and target, driven from the keyboard — and rebuilds
it on a modern foundation: memory-safe Rust, a resumable copy engine that
survives a `kill -9`, a typed theming system, and first-class mouse support.
It looks like the blue-screen file managers you remember, and behaves like
software written this decade.

## At a glance

| | |
|---|---|
| **Status** | Phase 1 shipped + Feature 031 (visual & interactive parity) + Feature 037 (resume-on-launch + SC-002 binary gate) + Feature 038 (quick-cd popup) + Feature 033 (panel filter prompt) + Feature 039 (tasks/jobs panel) + Feature 041 (in-session mouse-capture toggle) + Feature 042 (directory hotlist / bookmarks) |
| **Tests** | 276 unit + 9 integration, all green (+ a gated binary-level SC-002 SIGKILL-resume PTY test, enforced in CI) |
| **Binary** | 2.59 MiB stripped (ceiling: 8 MiB) |
| **Quality** | `clippy -D warnings` clean · CI green · TDD-gated |
| **Language** | Rust workspace (6 crates), `ratatui` + `crossterm` + `tokio` |
| **License** | MIT OR Apache-2.0 |

## Why cargonaut

The dual-pane ("orthodox") file manager is a forty-year-old idea that nothing
has improved on for keyboard-driven file work: two directory panels side by
side, one the source and one the target, every operation a single keystroke —
copy, move, delete, mkdir — no mouse round-trips, no path retyping, no modal
detours. It was the daily driver of the DOS era and still has a devoted
following on Linux.

But the beloved implementations are aging C codebases, and the terminal world
mostly forgot the paradigm in favor of `cd`-and-`ls`. Cargonaut's bet: the
interaction model is still the best one — what it needed was a 2026 engine.
So Cargonaut keeps the muscle memory and adds:

- **Crash-safe transfers** — every copy/move checkpoints with CRC chains and
  verifies SHA-256; pull the power mid-copy and it resumes from the last
  checkpoint instead of starting over.
- **A typed theme system** — a real palette (signature blue panels, colored
  directories/executables/symlinks, a cyan selection bar), not reverse-video
  hacks; switchable by name, legible down to 16 colors.
- **Mouse on by default** — click to focus, double-click to enter, wheel to
  scroll, click the menu and function-key bars — while every keystroke still
  works exactly as it always did.
- **Memory safety + tiny footprint** — no `unsafe` in the core crates, a
  ~2 MiB static binary, ≤ 64 MiB resident, sub-150 ms cold start.

```text
┌─[1: ~/projects/cargonaut ●]──────────┬─[2: /tmp ]───────────────────────────┐
│ ../                                  │ ../                                  │
│ .git/                       512 B    │ ash-XXVKE2/             4 K          │
│ Cargo.toml             1234 B  May17 │ runtime-1000/           4 K          │
│ README.md             3210 B  May17 │ snap-private-tmp/                    │
│ src/                                 │ vscode-ipc-...                       │
│ ─ 124,567 B   8 files                │ ─ 192 B   7 files                   │
└──────────────────────────────────────┴──────────────────────────────────────┘
 ~/projects/cargonaut/Cargo.toml | 1234 B | rw-r--r-- | 2026-05-17 14:23:01
[F1]help [F3]view [F4]edit [F5]copy [F6]move [F7]mkdir [F8]del [F10]quit
```

## Heritage & constellation

Cargonaut isn't a one-off — it's where several threads of
[Mohiuddin Khan Inamdar](https://github.com/mohnkhan)'s work converge. The
ethos across all of them: *systems built from the metal up, and little things
made to think.*

| Project | What it is | How it connects |
|---|---|---|
| [**Turbo_C_and_CPP**](https://github.com/mohnkhan/Turbo_C_and_CPP) | A book on DOS 6.22 / Turbo C / Turbo C++ programming | The **heritage** — the blue-screen, keyboard-first, dual-pane file managers of that era are exactly what Cargonaut revives. |
| [**MyOS2026**](https://github.com/mohnkhan/MyOS2026) | A VM-first experimental OS in Rust (sub-2 s boot, 400+ Linux-compatible syscalls) | The **engineering DNA** — Cargonaut inherits MyOS2026's spec-kit workflow and its multi-principle constitution verbatim. |
| [**ReduceSSDWrites**](https://github.com/mohnkhan/ReduceSSDWrites) | A measured guide to cutting Linux SSD wear via tmpfs redirection | The **discipline** — its tmpfs pattern became Cargonaut's NON-NEGOTIABLE Constitution §V (build artifacts live in tmpfs, never on the SSD). |
| [**awesome-operating-systems**](https://github.com/mohnkhan/awesome-operating-systems) | A curated catalog of 50+ open-source operating systems | The **context** — the systems-design tradition Cargonaut and MyOS2026 both sit inside. |

The orthodox file manager was born in the DOS world Inamdar documented; its
engineering rigor came from building an OS in Rust; its respect for the
developer's hardware came from measuring SSD wear. Cargonaut is the place those
meet.

## What works today

Phase 1 plus Feature 031 are merged — Cargonaut is a runnable dual-pane TUI:

- **Themed dual panes** with type-colored entries, a cursor bar, and tagged-file
  highlighting; `commander-dark` (default) and `monochrome` built-in themes.
- **Full chrome** — top pull-down menu bar, bottom `1Help…10Quit` function-key
  bar, per-pane mini-status (perms/size/mtime/name), F1 help.
- **Mouse** — focus/move/enter/scroll and clickable menu + F-key buttons.
- **Rich listings** — name/size/mtime/permission columns, brief vs full layout,
  quick-view file preview, sort cycling, recursive directory size.
- **Operations** — copy/move/delete with a confirm + live progress overlay
  (throughput + ETA, cancellable), mkdir, tag-by-glob, and F3/F4 view/edit via
  `$PAGER`/`$EDITOR`.
- **Crash-safe engine underneath** — resumable transfers, directory history,
  cd-on-exit shell integration.
- **Quick-cd prompt** (Alt-c) — inline path entry prefilled with the current
  directory, Tab-completing against the pane's directories and recent history,
  Enter to jump (Feature 038).
- **Panel filter prompt** (Alt-!) — inline prompt (prefilled with the active
  filter) to narrow the focused pane by glob (`*.rs`) or bare-word substring,
  case-insensitive; empty submit clears, invalid patterns show an inline error
  (Feature 033).
- **Tasks/jobs panel** (F12) — a modal list of the session's transfers showing
  each one's source → destination and live state/progress, with per-row
  cancel (`c`), pause (`p`), and resume (`r`); pause holds a transfer on its
  checkpoint while the others keep running, and resume picks it up where it left
  off (Feature 039).
- **In-session mouse-capture toggle** (`Alt-m`) — suspend/resume mouse capture
  at runtime without restarting, so you can drop to your terminal's native
  text selection (Shift+drag while capture is on) and pick back up. A menu-bar
  indicator shows the live state (`[mouse:on]` / `[mouse:susp]` / `[mouse:off]`);
  `--no-mouse` / `ui.mouse=false` keeps it off for the whole session (Feature 041).
- **Directory hotlist / bookmarks** (`Ctrl-b`) — a popup of named directory
  shortcuts organized by group; select to jump the active pane, `[a]`dd the
  current directory (prompting `group/name`), `[d]`elete an entry. Bookmarks
  persist to `~/.local/state/cargonaut/hotlist.toml` across sessions (Feature 042).

The full per-feature history (Features 001 → 042) lives in
[`CHANGELOG.md`](./CHANGELOG.md).

## Quick start

```bash
git clone https://github.com/mohnkhan/cargonaut
cd cargonaut
cargo build --release                    # debug build: cargo build
./target/release/cargonaut ~ /tmp        # left pane, right pane
```

**Install it:** `cargo install --git https://github.com/mohnkhan/cargonaut cargonaut`
(to `~/.cargo/bin`), or `sudo make install` (to `/usr/local/bin`). For a
dependency-free, runs-anywhere Linux binary: `make static`.

Try `--theme monochrome` or `--no-mouse`. Full build profiles (debug / release /
static), per-OS install, shell integration, and the make-target reference are in
[`docs/BUILD.md`](./docs/BUILD.md).

### Developer Quick Start

If you want to contribute or build from source, set up your development environment in seconds:

```bash
# 1. Redirect target/ to RAM to preserve your SSD (Constitution §V)
make tmpfs-setup

# 2. Run all tests to verify your environment
make test

# 3. Run the local CI pipeline (lints, tests, release build, size checks)
make ci-local
```
See [CONTRIBUTING.md](file:///home/main/MyOS-2026/cargonaut/CONTRIBUTING.md) for our mandatory branch workflows and commit style guidelines.


## How it's engineered

Cargonaut is built under the same governance as MyOS2026 — a spec-kit workflow
(specify → clarify → plan → tasks → analyze → implement) feeding a five-principle
[constitution](./.specify/memory/constitution.md):

1. **Code quality** — `clippy -D warnings`, `missing_docs` on every crate, no undocumented `unsafe`.
2. **Test-first** (NON-NEGOTIABLE) — a failing test is committed before the code that makes it pass.
3. **UX consistency** — one keymap source of truth; typed theme variables; a plain-text a11y event stream.
4. **Performance** (NON-NEGOTIABLE) — SC/NFR gates enforced by CI benches; a >10% regression blocks merge.
5. **SSD preservation** (NON-NEGOTIABLE, dev host) — `target/` lives in tmpfs (from [ReduceSSDWrites](https://github.com/mohnkhan/ReduceSSDWrites)).

## Documentation map

| Topic | Where |
|---|---|
| Build, run, test, make targets, CI, gates | [`docs/BUILD.md`](./docs/BUILD.md) |
| Per-feature changelog (001 → 031) | [`CHANGELOG.md`](./CHANGELOG.md) |
| Engineering retrospectives (what was hard & why) | [`Learnings.md`](./Learnings.md) |
| Forward-looking, issue-backed roadmap | [`ROADMAP.md`](./ROADMAP.md) |
| Architecture & full design tunnel | [`docs/architecture.md`](./docs/architecture.md) · [`design/INDEX.md`](./design/INDEX.md) |
| Contributing conventions | [`CONTRIBUTING.md`](./CONTRIBUTING.md) |

## Contributing

We welcome contributions of all kinds—bug fixes, new features, documentation, and code reviews! 

To get started:
1. **Explore the Roadmap**: Check out [ROADMAP.md](file:///home/main/MyOS-2026/cargonaut/ROADMAP.md) for prioritized tasks. Look for issues labeled `good first issue` for great starting points.
2. **Read the Guide**: Review our [CONTRIBUTING.md](file:///home/main/MyOS-2026/cargonaut/CONTRIBUTING.md) for code styling, TDD (Test-First) workflow, and git branch rules.
3. **Spec out Non-Trivial Work**: For substantial features, we write a design spec first using our `spec-kit` format. Read more in the [Spec-kit Workflow](file:///home/main/MyOS-2026/cargonaut/CONTRIBUTING.md#spec-kit-pattern-for-non-trivial-features) section of our contributing guide.

*Have a question or want to discuss a design? Feel free to open a GitHub issue!*

## License

Dual-licensed under **MIT OR Apache-2.0** — pick whichever fits your project.
See [`LICENSE-MIT`](./LICENSE-MIT) and [`LICENSE-APACHE`](./LICENSE-APACHE).

