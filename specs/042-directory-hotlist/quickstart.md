# Quickstart / Validation: Directory Hotlist / Bookmarks

**Feature**: 042-directory-hotlist | **Date**: 2026-06-15

How to validate end-to-end. Design details: [plan.md](./plan.md),
[data-model.md](./data-model.md),
[contracts/hotlist-seam.md](./contracts/hotlist-seam.md).

## Prerequisites

- tmpfs target active: `make tmpfs-status` (`make tmpfs-setup` if not).

## Automated validation

```bash
make ci-local                       # fmt, clippy -D warnings, test, release, docs-gate
# or, focused:
cargo test -p cargonaut-config hotlist
cargo test -p cargonaut-core bookmark
cargo test -p cargonaut-ui-tui hotlist
```

Expected new tests pass: config round-trip/absent/malformed/grouped + path
resolution; core add/remove/jump (incl. missing-target retain); popup
open/empty-state/select/add/remove + grouped render + help text.

## Manual validation (the SC walkthrough)

Use a throwaway state file so your real hotlist is untouched:

```bash
export XDG_STATE_HOME=$(mktemp -d)
cargo run -p cargonaut-bin -- ~ /tmp
```

1. **Add + jump (US1, SC-001/003)** — navigate the active pane into a directory.
   Press **Ctrl-b** to open the hotlist, use the **add** key, enter a name (or
   `work/myproj` to put it in the "work" group). Move the pane elsewhere, press
   **Ctrl-b**, select the bookmark → the active pane jumps to it.

2. **Persistence (US2, SC-002)** — quit (**F10**), relaunch with the same
   `XDG_STATE_HOME`, press **Ctrl-b** → the bookmark is still listed with its
   name/group. Inspect `"$XDG_STATE_HOME"/cargonaut/hotlist.toml` to see the
   saved TOML.

3. **Remove (US3, SC-005)** — open the hotlist, highlight an entry, use the
   **remove** key → it disappears; reopen (and relaunch) → still gone.

4. **Grouping (US4, SC-007)** — add bookmarks with `work/...` and `cfg/...`
   names; open the hotlist → entries appear under their group headers; a
   bookmark added with a bare name appears under the default/ungrouped section.

5. **Missing target (FR-008, SC-004)** — bookmark a directory, delete that
   directory outside the app, then select the bookmark → a clear status, panes
   unchanged, and the bookmark is still in the list (not auto-removed).

6. **Empty state (FR-010, SC-006)** — with a fresh `XDG_STATE_HOME`, press
   **Ctrl-b** before adding anything → the popup opens and clearly indicates
   there are no bookmarks (no dead-end).

7. **Malformed file (FR-013)** — write garbage into
   `"$XDG_STATE_HOME"/cargonaut/hotlist.toml`, relaunch → app starts with an
   empty hotlist and a non-fatal notice, no crash.

8. **Docs (help)** — press **F1**; the overlay mentions `Ctrl-b` and the
   add/remove keys.
```
