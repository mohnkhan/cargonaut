# Research — Feature 062 (survivability follow-ups)

## R1 — Input-handler recovery via async catch_unwind

**Decision**: Wrap `handle_key(..)` / `handle_mouse(..)` with
`AssertUnwindSafe(fut).catch_unwind().await`. On `Ok(inner)` propagate the inner
`Result`/quit; on `Err(payload)` drain `diag::take_captured_panic()`, set a
"recovered from internal input error" status, increment a consecutive-input-panic
counter, and `continue`; reset the counter on a clean event; escalate to
`Error::FatalPanic` after 3 (mirrors the render boundary, R7 of Feature 061).

**Rationale**: Same proven shape as render recovery; `AssertUnwindSafe` is sound
because after a caught input fault we return to the loop head and fully re-render
from `app` state next frame. The bound prevents a hot loop.

**Alternatives**: per-handler internal catches (scattered) — rejected; one
boundary at the call site is simplest.

## R2 — Transfer task → Failed on panic

**Decision**: Share the `watch::Sender<TransferState>` as `Arc<watch::Sender<_>>`
between `run_transfer` and a thin spawn wrapper. The wrapper does
`AssertUnwindSafe(run_transfer(.., tx.clone(), ..)).catch_unwind().await`; on
`Err`, if the last observed state is not already terminal, `tx.send(Failed {
error: "internal error (task panicked)", resumable: false })`. `Sender::send`
takes `&self`, so existing `state_tx.send(..)` calls inside `run_transfer` compile
unchanged after the param type becomes `Arc<watch::Sender<_>>`.

**Rationale**: `watch::Sender` is not `Clone`, but `Arc<Sender>` gives both the
task body and the wrapper a sending handle with zero behavioral change to the
many existing send sites. Detecting "already terminal" avoids overwriting a real
`Completed`/`Failed`/`Cancelled` with the generic panic failure.

**Alternatives**: detect sender-drop on the receiver side (UI marks Failed when
the channel closes mid-run) — more invasive across the registry/UI; rejected for
the localized wrapper.

## R3 — About view

**Decision**: UI-only keymap `Command::ShowAbout` (serde kebab `show-about`),
handled in `dispatch_ui_command` to set `ActiveDialog::About(AboutDialog)`;
`AboutDialog::render` centers `diag::about_lines()`; Esc/Enter closes (same
lifecycle as other modals). Menu entry in `chrome.rs`. No `keymap.toml` binding.

**Rationale**: Mirrors `ShowHelp`/`ShowUserMenu`; reuses the single
`diag::about()` source so all three About surfaces stay consistent; skipping a key
binding keeps `keymap.toml` + `help_covers_all_keymap_bindings` untouched.

**Alternatives**: a bound key — deferred (menu + F1 Help already give two paths).

## R4 — Unwrap audit scope

**Decision**: Bounded, reviewed shortlist on normal-operation hot paths (e.g.
listing/refresh, attribute reads, fsops) where an expected runtime condition (a
file vanishing between listing and stat, a non-UTF-8 name, a races read) could
panic. Convert to `?`/match returning `AppError` or a logged degrade. Tests where
practical. Not an exhaustive removal (many `unwrap`s are on provably-Ok values or
in tests).

**Rationale**: Maximizes panic-surface reduction per effort while keeping the
change reviewable and behavior-preserving on success paths.

No `NEEDS CLARIFICATION` remain.
