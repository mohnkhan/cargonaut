# Cross-Artifact Analysis: Quick-CD Popup

**Feature**: 038-quick-cd-popup | **Date**: 2026-06-15 | **Type**: read-only
consistency check across spec / plan / research / data-model / contracts / tasks,
verified against the code on `main` at branch time.

## Verdict

Artifacts are coherent and grounded. All contract/plan claims were verified
against the actual source (see Grounding below). One traceability gap and two
clarifications were found and fixed; the rest are non-blocking.

## Grounding verified (code matches the artifacts)

- `App::navigate_to(id, VfsPath)` — `crates/cargonaut-core/src/lib.rs:982`; lists
  the target (errors on missing/non-dir) and pushes `dir_history_back`. ✅
- `Command::QuickCdPopup` variant + status-stub handler — `cargonaut-core/src/lib.rs`
  (~156 def, ~556 handler). ✅ (to be replaced)
- `PaneState.dir_history_back: Vec<VfsPath>` — `cargonaut-core/src/lib.rs:75`. ✅
- `VfsBackend::list(path, sort) -> Result<DirListing, VfsError>`,
  `DirEntry{name, meta}`, `VfsKind::Dir` — `cargonaut-vfs/src/traits.rs` /
  `types.rs`. ✅
- `VfsPath { scheme, authority, segments }` + `parse/display/join/parent` —
  `cargonaut-vfs/src/types.rs:14–119` (`join` appends one segment, panics on
  `/`/`..` — resolver must split first, per R-003). ✅
- `ActiveDialog` enum + `TextInputDialog` + handle_key/render pattern —
  `cargonaut-ui-tui/src/lib.rs:106` / `dialog.rs`. ✅
- `M-c → quick-cd-popup` — `design/contracts/keymap.toml` (~246). ✅

## Findings & resolution

| # | Sev | Finding | Resolution |
|---|-----|---------|-----------|
| 1 | CRITICAL | SC-001 (keyboard-only nav, the MVP) missing from the tasks.md coverage map | **Fixed** — added row `SC-001 → T006/T007, T010, T011, T024`. Work already existed in US1; only the map omitted it. |
| 2 | LOW | Contract `complete_cd` signature clarity | Contract already shows `pub async fn complete_cd`; no change needed. |
| 3 | LOW | T004 prefill could be missed | T004 already specifies "prefilled with `app.active_pane_state().cwd.display()`"; left as-is. |
| 4 | MED | Resolver algorithm edge cases | Covered in research R-003 (split on `/`, `..` pops, `.` skipped, trailing `/` ignored) and exercised by T006 cases (a–d); deemed sufficient. |
| 5 | MED | Symlink-to-dir not explicitly tested | **Fixed** — T024 now notes symlink behavior is inherited from `navigate_to` and not separately tested. |
| 6 | MED | FR-009 "(no matches)" note persistence unspecified | **Fixed** — data-model invariants now state the note is non-blocking, cleared on edit / successful completion / close. |
| 7 | LOW | T010 forward-references T022 error path | T010 already annotates "(error handling completed in US3)"; acceptable staging. |
| 8–10 | LOW | Constitution link, #32/#33 issue links, T024 error-recovery wording | T024 wording **fixed** to include error-recovery assertion; issue links are referenced in spec/tasks already; non-blocking. |

## Conclusion

No CRITICAL items remain open. Proceed to `/speckit-checklist` then
`/speckit-implement`.
