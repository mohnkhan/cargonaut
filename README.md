# Cargonaut

> **The orthodox dual-pane file manager, rebuilt in Rust for 2026.**
> Keyboard-first. Mouse-friendly. Crash-safe transfers. ~2.6 MiB, no runtime.

[![CI](https://github.com/mohnkhan/cargonaut/actions/workflows/ci.yml/badge.svg)](https://github.com/mohnkhan/cargonaut/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/built%20with-Rust-orange.svg)](https://www.rust-lang.org)
[![Binary](https://img.shields.io/badge/binary-2.68%20MiB-success.svg)](#at-a-glance)
[![Tests](https://img.shields.io/badge/tests-366%20unit%20%2B%2012%20integration-brightgreen.svg)](#at-a-glance)

Cargonaut brings back the fastest way ever invented to move files around a
machine — two panes, source and target, driven from the keyboard — and rebuilds
it on a modern foundation: memory-safe Rust, a resumable copy engine that
survives a `kill -9`, a typed theming system, and first-class mouse support.
It looks like the blue-screen file managers you remember, and behaves like
software written this decade.

- 🛟 **Crash-safe transfers** — checkpointed, SHA-256-verified, resume after a `kill -9`.
- 🎨 **Typed theme system** — real palettes (not reverse-video hacks), legible to 16 colors.
- 🖱️ **Mouse on by default** — click / double-click / wheel / clickable bars — every keystroke still works.
- 🪶 **Tiny & safe** — one ~2.6 MiB static binary, no runtime, no `unsafe` in the core crates.

> **Status: alpha.** Cargonaut is a runnable, daily-usable dual-pane manager for
> the **local filesystem**. Remote backends (SFTP/S3/archives), an internal
> viewer/editor, and find-file are designed and [on the roadmap](#not-yet--on-the-roadmap).

**Jump to:** [Why](#why-cargonaut) · [What works](#what-works-today) · [Quick start](#quick-start) · [Keys](#essential-keys) · [Engineering](#how-its-engineered) · [Docs](#documentation-map) · [Contributing](#contributing)

## At a glance

| | |
|---|---|
| **Status** | Alpha · Phase 1 shipped, plus 12 features since (031–046) — see [`CHANGELOG.md`](./CHANGELOG.md) |
| **Tests** | 366 unit + 12 integration, all green (SC-002 SIGKILL-resume + SC-004 PTY navigation, both gated behind `CARGONAUT_PTY_TESTS=1`, enforced in CI) |
| **Binary** | 2.68 MiB stripped (ceiling: 8 MiB) |
| **Quality** | `clippy -D warnings` clean · CI green · TDD-gated |
| **Language** | Rust workspace (6 crates), `ratatui` + `crossterm` + `tokio` |
| **Platform** | Linux terminal (local filesystem) |
| **License** | MIT OR Apache-2.0 |

```text
 Left   File   Command   Options   Right                             [mouse:on]
┌─[1: ~/projects/cargonaut ●]──────────┬─[2: /tmp ]──────────────────────────┐
│ ../                                  │ ../                                 │
│ .git/                        512 B   │ ash-XXVKE2/             4 K         │
│ Cargo.toml              1234 B  Jun15 │ runtime-1000/           4 K        │
│ README.md               3210 B  Jun15 │ snap-private-tmp/                  │
│ src/                                 │ vscode-ipc-...                      │
│ ─ 124,567 B   8 files                │ ─ 192 B   7 files                  │
└──────────────────────────────────────┴─────────────────────────────────────┘
 ~/projects/cargonaut/Cargo.toml | 1234 B | rw-r--r-- | 2026-06-15 14:23:01
 1Help 2Menu 3View 4Edit 5Copy 6RenMov 7Mkdir 8Delete 9PullDn 10Quit
```

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
  ~2.6 MiB static binary, and CI-gated budgets of ≤ 64 MiB resident and
  sub-150 ms cold start.

## What works today

Phase 1 plus Features 031–045 are merged — Cargonaut is a runnable dual-pane TUI
for the local filesystem:

- **Themed dual panes** with type-colored entries, a cursor bar, and tagged-file
  highlighting; `commander-dark` (default) and `monochrome` built-in themes.
- **Full chrome** — top pull-down menu bar, bottom `1Help…10Quit` function-key
  bar, per-pane mini-status (perms/size/mtime/name), F1 help.
- **Mouse** — focus/move/enter/scroll and clickable menu + F-key buttons.
- **Rich listings** — name/size/mtime/permission columns, brief vs full layout,
  quick-view file preview, sort cycling, recursive directory size.
- **Operations** — copy/move/delete with a confirm + live progress overlay
  (throughput + ETA, cancellable), mkdir, tag-by-glob, and F3/F4 to view/edit a
  file via your `$PAGER`/`$EDITOR`.
- **Crash-safe engine underneath** — resumable transfers, directory history,
  cd-on-exit shell integration.
- **Quick-cd prompt** (`Alt-c`) — inline path entry prefilled with the current
  directory, Tab-completing against the pane's directories and recent history,
  Enter to jump (Feature 038).
- **Panel filter prompt** (`Alt-!`) — inline prompt (prefilled with the active
  filter) to narrow the focused pane by glob (`*.rs`) or bare-word substring,
  case-insensitive; empty submit clears, invalid patterns show an inline error
  (Feature 033).
- **Tasks/jobs panel** (`F12`) — a modal list of the session's transfers showing
  each one's source → destination and live state/progress, with per-row
  cancel (`c`), pause (`p`), and resume (`r`); pause holds a transfer on its
  checkpoint while the others keep running, and resume picks it up where it left
  off (Feature 039).
- **`..` parent row** — every non-root pane shows a `..` row as its first row;
  press Enter on it or double-click it to go up. It can't be tagged and is never
  part of a copy/move/delete; it's hidden at a filesystem root (Feature 040).
- **In-session mouse-capture toggle** (`Alt-m`) — suspend/resume mouse capture
  at runtime without restarting, so you can drop to your terminal's native
  text selection (Shift+drag while capture is on) and pick back up. A menu-bar
  indicator shows the live state (`[mouse:on]` / `[mouse:susp]` / `[mouse:off]`);
  `--no-mouse` / `ui.mouse=false` keeps it off for the whole session (Feature 041).
- **Directory hotlist / bookmarks** (`Ctrl-b`) — a popup of named directory
  shortcuts organized by group; select to jump the active pane, `[a]`dd the
  current directory (prompting `group/name`), `[d]`elete an entry. Bookmarks
  persist to `~/.local/state/cargonaut/hotlist.toml` across sessions (Feature 042).
- **File attributes** — change permissions (`C-x c` chmod, octal `755` or symbolic
  `u+x`), ownership (`C-x o` chown, with confirmation), and create symbolic
  (`C-x s`) and hard (`C-x l`) links, on the tagged/focused selection; also in the
  File menu. Errors (permission denied, bad mode) are reported, never crash
  (Feature 043).
- **Recursive file attributes** — `C-x C` (chmod) and `C-x O` (chown) apply a
  permission/ownership change to a whole directory subtree after a confirmation;
  a bounded walk (never following symlinks) changes entries deepest-first so a
  restrictive mode can't lock the walk out, with per-entry failures reported
  (Feature 044).
- **PTY navigation smoke test** — three binary-level integration tests (`nav_cursor_arrow_keys`,
  `nav_descend_enter`, `nav_ascend_backspace`) launch the real cargonaut binary under a PTY
  and assert that arrow keys, Enter (descend), and Backspace (ascend) produce the expected TUI
  output. Gated behind `CARGONAUT_PTY_TESTS=1` (SC-004); replaces the previous `#[ignore]`d
  stub; zero ignored tests remain in `cargonaut-bin` (Feature 045).
- **External theme skin files** — TOML skin files at `~/.config/cargonaut/themes/<name>.toml`
  (XDG_CONFIG_HOME honored) map any of the 25 theme element names to named 16-color, 256-color
  index, or RGB hex values; partial skins inherit from `commander-dark`; load errors (bad file,
  unknown field, invalid color) fall back to `commander-dark` with a one-line status message —
  no crash (Feature 046, closes #49).

The full per-feature history (Features 001 → 046) lives in
[`CHANGELOG.md`](./CHANGELOG.md).

### Not yet — on the roadmap

Cargonaut is alpha and honest about it. These are designed and issue-tracked but
**not implemented yet** (a few have keymap entries reserved that currently do
nothing):

- Remote & archive VFS backends — SFTP / S3 / archives-as-directories (#48)
- Internal file viewer with hex + search (`F3` today shells out to `$PAGER`) (#39)
- Internal full-screen editor (`F4` today shells out to `$EDITOR`) (#40)
- Find-file by name/content + external panelize (#41)
- Tabs (#45), undo + bulk-rename-via-editor (#47), persistent subshell (#44)
- Directory compare + diff (#43), the F2 user menu (#50)

The `list-plugins`, `audit`, and `resume` subcommands are placeholders today.
See [`ROADMAP.md`](./ROADMAP.md) — the authoritative, issue-backed plan.

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

### Essential keys

| Key | Action | | Key | Action |
|---|---|---|---|---|
| `↑`/`↓` `j`/`k` | move cursor | | `F5` | copy |
| `Enter` | enter dir / open | | `F6` | move / rename |
| `Backspace` `h` | go up (`..`) | | `F7` | mkdir |
| `Tab` | switch pane | | `F8` | delete |
| `Insert` | tag file | | `Alt-c` | quick-cd |
| `+` / `-` | tag / untag by glob | | `Alt-!` | filter pane |
| `Ctrl-b` | directory hotlist | | `F12` | tasks/jobs panel |
| `F3` / `F4` | view / edit | | `Alt-m` | toggle mouse capture |
| `F1` | help | | `F10` | quit |

The complete, always-current keymap lives in
[`design/contracts/keymap.toml`](./design/contracts/keymap.toml) (the single
source of truth) and on the wiki's Keybindings Reference.

### Building from source

Contributing or hacking on Cargonaut? The dev workflow keeps build artifacts in
RAM to spare your SSD (Constitution §V):

```bash
make tmpfs-setup     # redirect target/ to tmpfs (one-time, per checkout)
make test            # cargo test --workspace
make ci-local        # the full pipeline: clippy, tests, release build, gates
```

See [`CONTRIBUTING.md`](./CONTRIBUTING.md) for the mandatory branch/PR workflow
and commit conventions, and [`docs/dev-tmpfs.md`](./docs/dev-tmpfs.md) for the
tmpfs rationale.

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

## How it's engineered

Cargonaut is built under the same governance as MyOS2026 — a spec-kit workflow
(specify → clarify → plan → tasks → analyze → implement) feeding a five-principle
[constitution](./.specify/memory/constitution.md):

1. **Code quality** — `clippy -D warnings`, `missing_docs` on every crate, no undocumented `unsafe`.
2. **Test-first** (NON-NEGOTIABLE) — a failing test is committed before the code that makes it pass.
3. **UX consistency** — one keymap source of truth; typed theme variables; a plain-text a11y event stream.
4. **Performance** (NON-NEGOTIABLE) — SC/NFR gates enforced by CI benches; a >10% regression blocks merge.
5. **SSD preservation** (NON-NEGOTIABLE, dev host) — `target/` lives in tmpfs (from [ReduceSSDWrites](https://github.com/mohnkhan/ReduceSSDWrites)).

The workspace is six crates: `cargonaut-bin` (CLI + boot), `cargonaut-core`
(UI-agnostic app/state machine), `cargonaut-ui-tui` (ratatui render + keymap +
dialogs), `cargonaut-transfer` (the resumable copy engine), `cargonaut-vfs`
(filesystem abstraction), and `cargonaut-config` (typed config + state).

## Documentation map

| Topic | Where |
|---|---|
| Build, run, test, make targets, CI, gates | [`docs/BUILD.md`](./docs/BUILD.md) |
| Per-feature changelog (001 → 045) | [`CHANGELOG.md`](./CHANGELOG.md) |
| Engineering retrospectives (what was hard & why) | [`Learnings.md`](./Learnings.md) |
| Forward-looking, issue-backed roadmap | [`ROADMAP.md`](./ROADMAP.md) |
| Architecture & full design tunnel | [`docs/architecture.md`](./docs/architecture.md) · [`design/INDEX.md`](./design/INDEX.md) |
| Contributing conventions | [`CONTRIBUTING.md`](./CONTRIBUTING.md) |
| User guide, keybindings & config reference | [Project Wiki](https://github.com/mohnkhan/cargonaut/wiki) |

## Contributing

Contributions of all kinds are welcome — bug fixes, features, docs, and reviews.

1. **Find something to work on** — browse [`ROADMAP.md`](./ROADMAP.md) and the
   [issue tracker](https://github.com/mohnkhan/cargonaut/issues). The roadmap's
   tiers are ordered by leverage; the "Not yet" list above is a good menu of
   sizeable features.
2. **Read the guide** — [`CONTRIBUTING.md`](./CONTRIBUTING.md) covers the code
   style, the Test-First (TDD) workflow, and the mandatory feature-branch + PR
   rules (all work lands through a `NNN-short-desc` branch and a green CI run).
3. **Spec out non-trivial work** — substantial features start with a design spec
   in our `spec-kit` format; see the
   [spec-kit pattern](./CONTRIBUTING.md#spec-kit-pattern-for-non-trivial-features)
   in the contributing guide.

Have a question or a design to discuss? Open a GitHub issue.

## License

Dual-licensed under **MIT OR Apache-2.0** — pick whichever fits your project.
See [`LICENSE-MIT`](./LICENSE-MIT) and [`LICENSE-APACHE`](./LICENSE-APACHE).
