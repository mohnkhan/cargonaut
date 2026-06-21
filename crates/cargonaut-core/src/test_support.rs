// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Shared unit-test fixtures (Feature 059 split). `#[cfg(test)]` only.

#[allow(unused_imports)]
pub(crate) use crate::*;
pub(crate) use cargonaut_vfs::VfsCaps;
#[allow(unused_imports)]
pub(crate) use sha2::{Digest, Sha256};
pub(crate) use tempfile::TempDir;
#[allow(unused_imports)]
pub(crate) use tokio::fs;

pub(crate) async fn make_app(td_left: &TempDir, td_right: &TempDir) -> App {
    let config = cargonaut_config::Config::default();
    App::new(
        config,
        td_left.path().to_str().unwrap(),
        td_right.path().to_str().unwrap(),
    )
    .await
    .unwrap()
}

#[cfg(unix)]
pub(crate) fn mode_of(p: &std::path::Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).unwrap().permissions().mode() & 0o777
}

pub(crate) fn entry_index(app: &App, name: &str) -> usize {
    app.pane(PaneId::Left)
        .listing
        .entries
        .iter()
        .position(|e| e.name.as_str() == name)
        .expect("entry present")
}

pub(crate) async fn app_with_three(td_l: &TempDir, td_r: &TempDir) -> App {
    for n in ["a", "b", "c"] {
        fs::write(td_l.path().join(n), b"").await.unwrap();
    }
    let mut app = make_app(td_l, td_r).await;
    app.refresh_active_pane().await.unwrap();
    app
}

/// Submit one throttled copy of `name` (sized `bytes`) from the left
/// pane to the right pane via the App, returning its id. The throttle
/// keeps the copy in flight long enough for deterministic pause/cancel
/// assertions.
pub(crate) async fn submit_one_copy(
    app: &mut App,
    td_l: &TempDir,
    name: &str,
    bytes: usize,
) -> TransferId {
    std::env::set_var("CARGONAUT_TRANSFER_THROTTLE_MIBPS", "8");
    fs::write(td_l.path().join(name), vec![0u8; bytes])
        .await
        .unwrap();
    // Re-list the left pane so the new file is visible, then select it.
    app.refresh_active_pane().await.unwrap();
    app.dispatch(Command::SelectByPattern(name.to_string()))
        .await
        .unwrap();
    app.confirm_copy().await.unwrap();
    app.dispatch(Command::UnselectByPattern(name.to_string()))
        .await
        .unwrap();
    *app.transfer_ids().last().unwrap()
}

/// Poll a job's status until it reaches a terminal state or `deadline`
/// elapses, yielding so background transfer tasks make progress.
pub(crate) async fn wait_status<F>(app: &App, id: TransferId, deadline_ms: u64, pred: F) -> bool
where
    F: Fn(&JobStatus) -> bool,
{
    let start = std::time::Instant::now();
    loop {
        if let Some(v) = app.job_views().into_iter().find(|v| v.id == id) {
            if pred(&v.status) {
                return true;
            }
        }
        if start.elapsed().as_millis() as u64 > deadline_ms {
            return false;
        }
        tokio::time::sleep(std::time::Duration::from_millis(40)).await;
    }
}

pub(crate) fn file_uri(p: &std::path::Path) -> String {
    format!("file://{}", p.to_str().unwrap())
}

/// Stage a genuinely-resumable checkpoint: write the full source, a
/// partial destination (first `bytes_written` bytes), and a matching
/// sidecar in `dst_dir`. Returns the destination file path. The
/// resulting offer validates (`source_unchanged` + `dest_intact`).
pub(crate) async fn stage_checkpoint(
    src_dir: &std::path::Path,
    dst_dir: &std::path::Path,
    name: &str,
    full: &[u8],
    bytes_written: usize,
    interval: usize,
) -> std::path::PathBuf {
    assert!(
        bytes_written % interval == 0,
        "checkpoint at interval boundary"
    );
    let src = src_dir.join(name);
    let dst = dst_dir.join(name);
    fs::write(&src, full).await.unwrap();
    fs::write(&dst, &full[..bytes_written]).await.unwrap();

    let prefix_len = full.len().min(1024 * 1024);
    let mut h = Sha256::new();
    h.update(&full[..prefix_len]);
    let src_sha256_prefix: [u8; 32] = h.finalize().into();

    let chunk_crcs: Vec<u32> = full[..bytes_written]
        .chunks(interval)
        .map(crc32fast::hash)
        .collect();

    let cp = cargonaut_transfer::TransferCheckpoint {
        version: cargonaut_transfer::TransferCheckpoint::VERSION,
        job_id: "11111111-1111-4111-8111-111111111111".into(),
        src_uri: file_uri(&src),
        src_size: full.len() as u64,
        src_sha256_prefix,
        dst_uri: file_uri(&dst),
        bytes_written: bytes_written as u64,
        chunk_crcs,
        chunk_size_bytes: interval as u64,
        created_at: 0,
        last_update_at: 0,
    };
    let sidecar = dst_dir.join(format!(".cargonaut-transfer-{}.json", cp.job_id));
    fs::write(&sidecar, serde_json::to_vec(&cp).unwrap())
        .await
        .unwrap();
    dst
}

pub(crate) async fn wait_completed(app: &App, id: TransferId) -> TransferState {
    let mut rx = app.transfer(id).unwrap().state.clone();
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        loop {
            {
                let s = rx.borrow();
                if matches!(
                    *s,
                    TransferState::Completed { .. }
                        | TransferState::Failed { .. }
                        | TransferState::Canceled
                ) {
                    return s.clone();
                }
            }
            if rx.changed().await.is_err() {
                return TransferState::Failed {
                    error: "sender dropped".into(),
                    resumable: false,
                };
            }
        }
    })
    .await
    .expect("transfer did not terminate in 30s")
}

pub(crate) async fn make_compare_app(td_l: &TempDir, td_r: &TempDir) -> App {
    make_app(td_l, td_r).await
}

pub(crate) async fn make_nested_app(td_parent: &TempDir, td_r: &TempDir) -> (App, String) {
    let inner = td_parent.path().join("inner");
    tokio::fs::create_dir_all(&inner).await.unwrap();
    let app = App::new(
        cargonaut_config::Config::default(),
        inner.to_str().unwrap(),
        td_r.path().to_str().unwrap(),
    )
    .await
    .unwrap();
    let inner_cwd = app.pane(PaneId::Left).cwd.display();
    (app, inner_cwd)
}
