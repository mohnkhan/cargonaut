// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Feature 059 split: `transfers` module of `cargonaut-core`.
//!
//! Moved verbatim from the former `lib.rs` god-file (move-only refactor).

#[allow(unused_imports)]
use crate::*;

impl App {
    /// Snapshot of active transfer ids in submit order.
    pub fn transfer_ids(&self) -> Vec<TransferId> {
        self.transfer_order.clone()
    }

    /// Borrow a transfer by id (for the UI to read its `watch::Receiver`).
    pub fn transfer(&self, id: TransferId) -> Option<&TransferJob> {
        self.transfers.get(&id)
    }

    /// Feature 039 (FR-002/003/004) — a read-only projection of every
    /// transfer in the registry, in submit order, for the tasks/jobs panel.
    /// Pure (no I/O). The user-paused marker overrides a raw `Canceled`
    /// snapshot so a deliberately-paused transfer renders as `Paused`; a
    /// transfer that reached a terminal state before the pause took effect
    /// still renders with its terminal status.
    pub fn job_views(&self) -> Vec<JobView> {
        self.transfer_order
            .iter()
            .filter_map(|id| {
                let job = self.transfers.get(id)?;
                let raw = transfer_state_snapshot(job);
                let paused = self.paused.contains(id);
                let status = job_status_from(raw, paused);
                Some(JobView {
                    id: *id,
                    src: job.src.1.display(),
                    dst: job.dst.1.display(),
                    mode: job.mode,
                    status,
                })
            })
            .collect()
    }

    /// Feature 039 (FR-009, SC-002) — cancel the transfer with `id` through
    /// the engine's cancellation token. Clears any user-paused marker first
    /// so the job renders as `Cancelled`, not `Paused`. Affects only this
    /// transfer; unknown ids are a safe no-op.
    pub fn cancel_transfer(&mut self, id: TransferId) -> Vec<Event> {
        self.paused.remove(&id);
        match self.transfers.get(&id) {
            Some(job) => {
                job.cancel.cancel();
                vec![Event::Status(format!("Canceled transfer {id:?}"))]
            }
            None => vec![Event::Status("No such transfer".into())],
        }
    }

    /// Feature 039 (FR-010/012/016/017) — pause the transfer with `id`.
    /// Signals its cancellation token (which stops the copy loop while
    /// leaving the checkpoint sidecar in place) and records the id as
    /// user-paused so it renders as `Paused` and is resume-eligible. A
    /// no-op on unknown, already-paused, or terminal transfers.
    pub fn pause_transfer(&mut self, id: TransferId) -> Vec<Event> {
        if self.paused.contains(&id) {
            return vec![];
        }
        let pausable = match self.transfers.get(&id) {
            None => return vec![],
            Some(job) => match transfer_state_snapshot(job) {
                TransferState::Completed { .. }
                | TransferState::Failed { .. }
                | TransferState::Canceled => false,
                _ => {
                    job.cancel.cancel();
                    true
                }
            },
        };
        if pausable {
            self.paused.insert(id);
            vec![Event::Status(format!("Paused transfer {id:?}"))]
        } else {
            vec![]
        }
    }

    /// Feature 039 (FR-011/012) — resume a user-paused transfer. Locates
    /// the job's checkpoint sidecar (via `scan_resumable` on the destination
    /// directory) and re-arms it through [`resume_transfer`] with a fresh
    /// cancellation token, preserving its `TransferId`. If no checkpoint
    /// exists yet (paused before the first checkpoint interval), the
    /// transfer is restarted from scratch. A no-op when `id` is not paused.
    pub async fn resume_paused(&mut self, id: TransferId) -> Result<Vec<Event>, AppError> {
        if !self.paused.contains(&id) {
            return Ok(vec![]);
        }
        let (src_path, dst_path) = match self.transfers.get(&id) {
            Some(job) => (job.src.1.clone(), job.dst.1.clone()),
            None => {
                self.paused.remove(&id);
                return Ok(vec![]);
            }
        };
        let dst_parent = dst_path
            .parent()
            .ok_or_else(|| AppError::BadPath("destination path has no parent".into()))?;
        let opts = self.transfer_opts();

        // Find this job's checkpoint sidecar in the destination directory.
        let found = scan_resumable(self.registry.local(), dst_parent)
            .await?
            .into_iter()
            .find(|rt| rt.checkpoint.job_id == id.0.to_string());

        match found {
            Some(rt) => {
                match resume_transfer(
                    self.registry.local(),
                    self.registry.local(),
                    rt.checkpoint,
                    opts,
                )
                .await
                {
                    Ok(job) => {
                        // resume_transfer preserves the id, so this replaces
                        // the paused entry in place; transfer_order is intact.
                        let new_id = job.id;
                        self.transfers.insert(new_id, job);
                        self.paused.remove(&id);
                        Ok(vec![Event::TransferProgressed(new_id)])
                    }
                    Err(e) => Ok(vec![Event::Status(format!("Cannot resume: {e}"))]),
                }
            }
            None => {
                // No checkpoint yet — restart from scratch. submit_transfer
                // mints a fresh id, so swap it into transfer_order in place.
                let job = submit_transfer(
                    self.registry.local(),
                    src_path,
                    self.registry.local(),
                    dst_path,
                    opts,
                )
                .await?;
                let new_id = job.id;
                self.transfers.insert(new_id, job);
                if let Some(pos) = self.transfer_order.iter().position(|x| *x == id) {
                    self.transfer_order[pos] = new_id;
                } else {
                    self.transfer_order.push(new_id);
                }
                self.transfers.remove(&id);
                self.paused.remove(&id);
                Ok(vec![Event::TransferProgressed(new_id)])
            }
        }
    }

    /// Actually start a copy from the active pane's selection (or focused
    /// entry) to the opposite pane's cwd. Caller invokes this *after* the
    /// user confirms the dialog.
    ///
    /// Feature 050 T017: records `UndoEntry::Copy` with the destination paths
    /// so the user can undo the copy via `C-z`.
    pub async fn confirm_copy(&mut self) -> Result<Vec<Event>, AppError> {
        let src_pane = self.active;
        let dst_pane = src_pane.other();
        let entries = self.selection_or_focused(src_pane);
        if entries.is_empty() {
            return Ok(vec![Event::Status("Nothing selected".into())]);
        }
        let dst_cwd = self.pane(dst_pane).cwd.clone();
        let opts = self.transfer_opts();
        let mut events = Vec::new();
        let mut copy_paths: Vec<VfsPath> = Vec::new();
        for entry_name in entries {
            let src_path = self.pane(src_pane).cwd.join(&entry_name);
            let dst_path = dst_cwd.join(&entry_name);
            copy_paths.push(dst_path.clone());
            let job = submit_transfer(
                self.registry.local(),
                src_path,
                self.registry.local(),
                dst_path,
                opts.clone(),
            )
            .await?;
            let id = job.id;
            self.transfers.insert(id, job);
            self.transfer_order.push(id);
            events.push(Event::TransferProgressed(id));
        }
        // Feature 050 T017: record copy destinations for undo.
        if !copy_paths.is_empty() {
            self.undo_log = Some(UndoEntry::Copy { copies: copy_paths });
        }
        Ok(events)
    }

    /// Transfer options derived from the active config (checkpoint
    /// interval + post-copy verification). Shared by fresh copies and
    /// resumed/started-over transfers so they behave identically.
    pub(crate) fn transfer_opts(&self) -> TransferOptions {
        TransferOptions {
            checkpoint_interval_bytes: u64::from(self.config.transfer.checkpoint_interval_mib)
                * 1024
                * 1024,
            verify_after_copy: self.config.transfer.verify_after_copy,
            ..Default::default()
        }
    }

    /// Scan both pane directories (non-recursively, de-duplicated) for
    /// orphan checkpoint sidecars and remember any found as pending resume
    /// offers. Returns UI-friendly projections in scan order. Safe to call
    /// once on launch; an empty result means "nothing to resume" (the hot
    /// path — no prompt). (FR-001/002/003)
    pub async fn scan_resume_offers(&mut self) -> Result<Vec<ResumeOfferView>, AppError> {
        self.pending_resumes.clear();
        let mut scanned: Vec<VfsPath> = Vec::new();
        for id in [PaneId::Left, PaneId::Right] {
            let dir = self.pane(id).cwd.clone();
            if scanned.contains(&dir) {
                continue;
            }
            scanned.push(dir.clone());
            let found = scan_resumable(self.registry.local(), dir).await?;
            self.pending_resumes.extend(found);
        }
        Ok(self.pending_resume_views())
    }

    /// Project the current pending resume offers to UI views, in order.
    /// Pure (no I/O). Used by the UI to rebuild its prompt after each
    /// choice.
    pub fn pending_resume_views(&self) -> Vec<ResumeOfferView> {
        self.pending_resumes.iter().map(resume_offer_view).collect()
    }

    /// Resume the offer at `index`: continue the transfer from its
    /// checkpoint and register it like any other in-flight transfer. On a
    /// validation failure (e.g. the destination changed) the offer is
    /// dropped and a status message is returned — never a corrupt copy.
    /// (FR-005/006/009, SC-005)
    pub async fn resume_offer(&mut self, index: usize) -> Result<Vec<Event>, AppError> {
        if index >= self.pending_resumes.len() {
            return Ok(vec![Event::Status("No such resume offer".into())]);
        }
        let rt = self.pending_resumes.remove(index);
        let opts = self.transfer_opts();
        match resume_transfer(
            self.registry.local(),
            self.registry.local(),
            rt.checkpoint,
            opts,
        )
        .await
        {
            Ok(job) => {
                let id = job.id;
                self.transfers.insert(id, job);
                self.transfer_order.push(id);
                Ok(vec![Event::TransferProgressed(id)])
            }
            Err(e) => Ok(vec![Event::Status(format!("Cannot resume: {e}"))]),
        }
    }

    /// Start the offer at `index` over from scratch: discard its
    /// checkpoint sidecar and submit a fresh copy (which truncates the
    /// partial destination). (FR-007)
    pub async fn start_over_offer(&mut self, index: usize) -> Result<Vec<Event>, AppError> {
        if index >= self.pending_resumes.len() {
            return Ok(vec![Event::Status("No such resume offer".into())]);
        }
        let rt = self.pending_resumes.remove(index);
        // Discard the stale checkpoint so a future scan won't re-offer it.
        let _ = std::fs::remove_file(&rt.checkpoint_path);
        let src = parse_path(&rt.checkpoint.src_uri)?;
        let dst = parse_path(&rt.checkpoint.dst_uri)?;
        let opts = self.transfer_opts();
        let job =
            submit_transfer(self.registry.local(), src, self.registry.local(), dst, opts).await?;
        let id = job.id;
        self.transfers.insert(id, job);
        self.transfer_order.push(id);
        Ok(vec![Event::TransferProgressed(id)])
    }

    /// Skip the offer at `index`: start no transfer and leave the
    /// checkpoint sidecar on disk so it is offered again next launch.
    /// (FR-008)
    pub fn skip_offer(&mut self, index: usize) {
        if index < self.pending_resumes.len() {
            self.pending_resumes.remove(index);
        }
    }

    pub(crate) fn request_copy_confirmation(&self) -> Result<Vec<Event>, AppError> {
        let names = self.selection_or_focused(self.active);
        if names.is_empty() {
            return Ok(vec![Event::Status("Nothing selected to copy".into())]);
        }
        let dst = self.pane(self.active.other()).cwd.display();
        let body = format!(
            "Copy {} item(s) to {dst}:\n  {}",
            names.len(),
            names.join("\n  ")
        );
        Ok(vec![Event::DialogRequested(DialogKind::Confirm {
            title: "Copy".into(),
            body,
            on_confirm: Box::new(Command::Copy),
        })])
    }

    pub(crate) fn request_move_confirmation(&self) -> Result<Vec<Event>, AppError> {
        let names = self.selection_or_focused(self.active);
        if names.is_empty() {
            return Ok(vec![Event::Status("Nothing selected to move".into())]);
        }
        let body = format!("Move {} item(s)", names.len());
        Ok(vec![Event::DialogRequested(DialogKind::Confirm {
            title: "Move".into(),
            body,
            on_confirm: Box::new(Command::Move),
        })])
    }

    pub(crate) fn request_delete_confirmation(&self) -> Result<Vec<Event>, AppError> {
        let names = self.selection_or_focused(self.active);
        if names.is_empty() {
            return Ok(vec![Event::Status("Nothing selected to delete".into())]);
        }
        let body = format!(
            "Permanently delete {} item(s)?\n  {}",
            names.len(),
            names.join("\n  ")
        );
        Ok(vec![Event::DialogRequested(DialogKind::Confirm {
            title: "Delete".into(),
            body,
            on_confirm: Box::new(Command::Delete),
        })])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use crate::test_support::*;

    #[tokio::test]
    async fn copy_with_no_selection_emits_status_not_dialog() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let events = app.dispatch(Command::Copy).await.unwrap();
        assert!(
            events.iter().any(|e| matches!(e, Event::Status(_))),
            "expected Status event, got {events:?}"
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, Event::DialogRequested(_))),
            "no dialog when nothing to copy"
        );
    }

    #[tokio::test]
    async fn copy_with_selection_requests_confirmation() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("a"), b"hello").await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.dispatch(Command::SelectionToggle).await.unwrap();
        let events = app.dispatch(Command::Copy).await.unwrap();
        let has_dialog = events
            .iter()
            .any(|e| matches!(e, Event::DialogRequested(DialogKind::Confirm { .. })));
        assert!(has_dialog, "expected Confirm dialog, got {events:?}");
    }

    #[tokio::test]
    async fn confirm_copy_spawns_a_transfer() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("a"), b"hello").await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.dispatch(Command::SelectionToggle).await.unwrap();
        app.dispatch(Command::Copy).await.unwrap(); // request dialog
        let events = app.confirm_copy().await.unwrap();
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::TransferProgressed(_))));
        assert_eq!(app.transfer_ids().len(), 1);
    }

    #[tokio::test]
    async fn show_tasks_panel_dispatch_is_noop() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        // The TUI intercepts ShowTasksPanel to open the modal; the core
        // dispatch arm is a no-op (like QuickCdPopup).
        let events = app.dispatch(Command::ShowTasksPanel).await.unwrap();
        assert!(events.is_empty(), "expected no events, got {events:?}");
    }

    #[tokio::test]
    async fn job_views_empty_when_no_transfers() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let app = make_app(&td_l, &td_r).await;
        assert!(app.job_views().is_empty());
    }

    #[tokio::test]
    async fn job_views_lists_transfers_in_submit_order() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let id_a = submit_one_copy(&mut app, &td_l, "a.bin", 16 * 1024 * 1024).await;
        let id_b = submit_one_copy(&mut app, &td_l, "b.bin", 16 * 1024 * 1024).await;
        let views = app.job_views();
        assert_eq!(views.len(), 2);
        assert_eq!(views[0].id, id_a);
        assert_eq!(views[1].id, id_b);
        assert!(views[0].src.contains("a.bin"));
        assert!(views[0].dst.contains("a.bin"));
        // A fresh, throttled copy is queued or running — never terminal yet.
        assert!(matches!(
            views[0].status,
            JobStatus::Running { .. } | JobStatus::Queued
        ));
    }

    #[tokio::test]
    async fn cancel_transfer_signals_only_that_job() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let id_a = submit_one_copy(&mut app, &td_l, "a.bin", 16 * 1024 * 1024).await;
        let id_b = submit_one_copy(&mut app, &td_l, "b.bin", 16 * 1024 * 1024).await;
        let events = app.cancel_transfer(id_a);
        assert!(events.iter().any(|e| matches!(e, Event::Status(_))));
        assert!(app.transfer(id_a).unwrap().cancel.is_cancelled());
        assert!(!app.transfer(id_b).unwrap().cancel.is_cancelled());
    }

    #[tokio::test]
    async fn cancel_transfer_unknown_id_is_safe_noop() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let bogus = TransferId(uuid::Uuid::nil());
        let _ = app.cancel_transfer(bogus); // must not panic
        assert!(app.job_views().is_empty());
    }

    #[tokio::test]
    async fn pause_transfer_marks_paused_and_cancels_token() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let id = submit_one_copy(&mut app, &td_l, "big.bin", 32 * 1024 * 1024).await;
        let _ = app.pause_transfer(id);
        // Token is signalled so the running task stops (leaving its checkpoint).
        assert!(app.transfer(id).unwrap().cancel.is_cancelled());
        // The throttled copy is still mid-flight, so it classifies as Paused.
        let v = app.job_views();
        assert!(matches!(v[0].status, JobStatus::Paused));
    }

    #[tokio::test]
    async fn pause_transfer_unknown_id_is_safe_noop() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let bogus = TransferId(uuid::Uuid::nil());
        let events = app.pause_transfer(bogus);
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn resume_paused_noop_when_not_paused() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let id = submit_one_copy(&mut app, &td_l, "a.bin", 16 * 1024 * 1024).await;
        // Never paused → resume is a no-op (no events).
        let events = app.resume_paused(id).await.unwrap();
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn cancel_transfer_clears_paused_marker() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let id = submit_one_copy(&mut app, &td_l, "big.bin", 32 * 1024 * 1024).await;
        let _ = app.pause_transfer(id);
        assert!(matches!(app.job_views()[0].status, JobStatus::Paused));
        let _ = app.cancel_transfer(id);
        // After cancel the user-paused marker is cleared, so the job no
        // longer classifies as Paused (it renders as Cancelled once the
        // task observes the token — never Paused again). The deterministic
        // invariant is "not Paused".
        assert!(
            !matches!(app.job_views()[0].status, JobStatus::Paused),
            "cancel must clear the paused marker"
        );
        assert!(app.transfer(id).unwrap().cancel.is_cancelled());
    }

    /// SC-003 (the issue's headline acceptance test): submit three throttled
    /// transfers, pause one, and assert the other two run to completion while
    /// the paused one is held; then resume it and assert it completes too.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn three_jobs_pause_one_others_continue() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::env::set_var("CARGONAUT_TRANSFER_THROTTLE_MIBPS", "16");
        for name in ["a.bin", "b.bin", "c.bin"] {
            fs::write(td_l.path().join(name), vec![0x5Au8; 24 * 1024 * 1024])
                .await
                .unwrap();
        }
        let mut app = make_app(&td_l, &td_r).await;
        app.refresh_active_pane().await.unwrap();
        app.dispatch(Command::SelectByPattern("*.bin".into()))
            .await
            .unwrap();
        app.confirm_copy().await.unwrap();

        let ids = app.transfer_ids();
        assert_eq!(ids.len(), 3, "expected 3 transfers, got {}", ids.len());
        let paused_id = ids[1];

        // Pause the middle job immediately (still in flight under throttle).
        let _ = app.pause_transfer(paused_id);
        assert!(app.transfer(paused_id).unwrap().cancel.is_cancelled());

        // The other two must complete.
        assert!(
            wait_status(&app, ids[0], 30_000, |s| matches!(
                s,
                JobStatus::Completed { .. }
            ))
            .await,
            "sibling 0 did not complete"
        );
        assert!(
            wait_status(&app, ids[2], 30_000, |s| matches!(
                s,
                JobStatus::Completed { .. }
            ))
            .await,
            "sibling 2 did not complete"
        );

        // The paused job did NOT complete — it is held as Paused.
        let paused_status = app
            .job_views()
            .into_iter()
            .find(|v| v.id == paused_id)
            .unwrap()
            .status;
        assert!(
            matches!(paused_status, JobStatus::Paused),
            "paused job should be Paused, was {paused_status:?}"
        );

        // Resume it and assert it completes.
        let _ = app.resume_paused(paused_id).await.unwrap();
        // resume_transfer preserves the id; the from-scratch fallback would
        // swap it, so resolve the (possibly new) middle id from order.
        let resumed_id = app.transfer_ids()[1];
        assert!(
            wait_status(&app, resumed_id, 30_000, |s| matches!(
                s,
                JobStatus::Completed { .. }
            ))
            .await,
            "resumed job did not complete"
        );

        // All three destinations exist and match the source size.
        for name in ["a.bin", "b.bin", "c.bin"] {
            let meta = std::fs::metadata(td_r.path().join(name)).unwrap();
            assert_eq!(meta.len(), 24 * 1024 * 1024, "{name} wrong size");
        }
    }

    #[tokio::test]
    async fn cancel_current_transfer_signals_cancel_on_latest() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let big: Vec<u8> = vec![0u8; 8 * 1024 * 1024];
        fs::write(td_l.path().join("big"), &big).await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.dispatch(Command::SelectionToggle).await.unwrap();
        app.dispatch(Command::Copy).await.unwrap();
        app.confirm_copy().await.unwrap();
        let id = app.transfer_ids()[0];
        let events = app.dispatch(Command::CancelCurrentTransfer).await.unwrap();
        assert!(events.iter().any(|e| matches!(e, Event::Status(_))));
        // The cancellation token must be triggered.
        assert!(app.transfer(id).unwrap().cancel.is_cancelled());
    }

    #[tokio::test]
    async fn pending_resume_views_empty_on_fresh_app() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let app = make_app(&td_l, &td_r).await;
        assert!(app.pending_resume_views().is_empty());
    }

    #[tokio::test]
    async fn scan_finds_offer_in_a_pane_dir() {
        let td_src = TempDir::new().unwrap();
        let td_dst = TempDir::new().unwrap();
        let full = vec![0xABu8; 4096];
        stage_checkpoint(td_src.path(), td_dst.path(), "big.bin", &full, 2048, 1024).await;
        // Right pane is the destination dir holding the sidecar.
        let mut app = make_app(&td_src, &td_dst).await;
        let offers = app.scan_resume_offers().await.unwrap();
        assert_eq!(offers.len(), 1, "expected one resumable offer");
        assert_eq!(app.pending_resume_views().len(), 1);
        assert!(offers[0].source_unchanged && offers[0].dest_intact);
    }

    #[tokio::test]
    async fn scan_finds_nothing_when_no_sidecars() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("plain.txt"), b"hi")
            .await
            .unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let offers = app.scan_resume_offers().await.unwrap();
        assert!(offers.is_empty());
        assert!(app.pending_resume_views().is_empty());
    }

    #[tokio::test]
    async fn scan_ignores_malformed_sidecar() {
        // FR-010: a garbage sidecar must not error or appear as an offer.
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(
            td_r.path().join(".cargonaut-transfer-bogus.json"),
            b"{ not valid json ",
        )
        .await
        .unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let offers = app.scan_resume_offers().await.unwrap();
        assert!(
            offers.is_empty(),
            "malformed sidecar must not yield an offer"
        );
    }

    #[tokio::test]
    async fn resume_offer_completes_and_matches_source() {
        let td_src = TempDir::new().unwrap();
        let td_dst = TempDir::new().unwrap();
        let full: Vec<u8> = (0..8192u32).map(|i| (i % 251) as u8).collect();
        let dst =
            stage_checkpoint(td_src.path(), td_dst.path(), "big.bin", &full, 4096, 1024).await;

        let mut app = make_app(&td_src, &td_dst).await;
        app.scan_resume_offers().await.unwrap();
        let events = app.resume_offer(0).await.unwrap();
        let id = match events.first() {
            Some(Event::TransferProgressed(id)) => *id,
            other => panic!("expected TransferProgressed, got {other:?}"),
        };
        assert!(app.pending_resume_views().is_empty(), "offer consumed");

        let final_state = wait_completed(&app, id).await;
        assert!(
            matches!(final_state, TransferState::Completed { sha256_match: true }),
            "expected Completed{{sha256_match:true}}, got {final_state:?}"
        );
        assert_eq!(fs::read(&dst).await.unwrap(), full, "dst must equal src");
    }

    #[tokio::test]
    async fn resume_offer_fails_safe_on_changed_destination() {
        // FR-009 / SC-005: if the partial destination no longer matches the
        // checkpoint, resume must refuse rather than corrupt it.
        let td_src = TempDir::new().unwrap();
        let td_dst = TempDir::new().unwrap();
        let full: Vec<u8> = (0..8192u32).map(|i| (i % 251) as u8).collect();
        let dst =
            stage_checkpoint(td_src.path(), td_dst.path(), "big.bin", &full, 4096, 1024).await;
        // Corrupt the partial destination after staging.
        fs::write(&dst, vec![0xFFu8; 4096]).await.unwrap();

        let mut app = make_app(&td_src, &td_dst).await;
        app.scan_resume_offers().await.unwrap();
        let events = app.resume_offer(0).await.unwrap();
        // No successful transfer was registered; a status explains why.
        assert!(
            app.transfer_ids().is_empty(),
            "no transfer should be registered on a fail-safe refusal"
        );
        assert!(
            events.iter().any(|e| matches!(e, Event::Status(_))),
            "expected a status message, got {events:?}"
        );
    }

    #[tokio::test]
    async fn start_over_discards_checkpoint_and_copies_fresh() {
        let td_src = TempDir::new().unwrap();
        let td_dst = TempDir::new().unwrap();
        let full: Vec<u8> = (0..8192u32).map(|i| (i % 251) as u8).collect();
        let dst =
            stage_checkpoint(td_src.path(), td_dst.path(), "big.bin", &full, 4096, 1024).await;
        let sidecar = td_dst
            .path()
            .join(".cargonaut-transfer-11111111-1111-4111-8111-111111111111.json");
        assert!(sidecar.exists());

        let mut app = make_app(&td_src, &td_dst).await;
        app.scan_resume_offers().await.unwrap();
        let events = app.start_over_offer(0).await.unwrap();
        let id = match events.first() {
            Some(Event::TransferProgressed(id)) => *id,
            other => panic!("expected TransferProgressed, got {other:?}"),
        };
        assert!(
            !sidecar.exists(),
            "start over must remove the stale sidecar"
        );
        assert!(app.pending_resume_views().is_empty());

        let final_state = wait_completed(&app, id).await;
        assert!(matches!(final_state, TransferState::Completed { .. }));
        assert_eq!(fs::read(&dst).await.unwrap(), full);
    }

    #[tokio::test]
    async fn skip_offer_starts_nothing_and_keeps_sidecar() {
        let td_src = TempDir::new().unwrap();
        let td_dst = TempDir::new().unwrap();
        let full = vec![0x33u8; 4096];
        stage_checkpoint(td_src.path(), td_dst.path(), "big.bin", &full, 2048, 1024).await;
        let sidecar = td_dst
            .path()
            .join(".cargonaut-transfer-11111111-1111-4111-8111-111111111111.json");

        let mut app = make_app(&td_src, &td_dst).await;
        app.scan_resume_offers().await.unwrap();
        app.skip_offer(0);
        assert!(app.transfer_ids().is_empty(), "skip starts no transfer");
        assert!(
            app.pending_resume_views().is_empty(),
            "offer dropped from memory"
        );
        assert!(sidecar.exists(), "skip leaves the sidecar on disk");

        // A fresh scan re-discovers the skipped transfer.
        let offers = app.scan_resume_offers().await.unwrap();
        assert_eq!(
            offers.len(),
            1,
            "skipped transfer is offered again next launch"
        );
    }
}
