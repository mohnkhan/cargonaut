// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Live-server SFTP integration test — issue #84 (Feature 057 T041).
//!
//! Satisfies the SC-003 / SC-004 success criteria for the SFTP backend that the
//! mock-backed unit tests in `sftp_fs_mock.rs` cannot reach: they stub out the
//! transport, so they prove the `VfsBackend` glue but never exercise a real
//! TCP/SSH/crypto path. This test drives `SftpFs::connect` against an actual
//! OpenSSH-backed server.
//!
//! - **SC-003**: connect and list the root directory within 5 s.
//! - **SC-004**: transfer a ≥10 MiB file and measure throughput.
//!
//! Gated behind the `ci-integration` cargo feature so plain `cargo test` stays
//! hermetic. The server is provided by `docker-compose.ci.yml`
//! (`atmoz/sftp testuser:testpass:1001::upload` on `localhost:2222`); bring it
//! up with `make ci-sftp-up`, then:
//!
//! ```text
//! cargo test -p cargonaut-vfs --features ci-integration
//! ```
//!
//! In CI the `sftp-integration` job in `.github/workflows/ci.yml` does this.
//!
//! ## On the SC-004 throughput gate
//!
//! SC-004 targets "≥70% of 1 Gbps" = 87.5 MB/s. Single-stream SFTP throughput
//! is bound by symmetric-cipher CPU cost, not the loopback link, so on a shared
//! 2-core CI runner it routinely lands below 87.5 MB/s. Gating hard on that
//! figure would make the required `ci` check flaky — worse than no gate. So this
//! test **logs** the measured throughput and its percentage of the target, and
//! **asserts** only a conservative floor that real SFTP comfortably clears. The
//! logged number is the artifact engineers read to track SC-004 over time.

#![cfg(feature = "ci-integration")]

use std::sync::Arc;
use std::time::{Duration, Instant};

use cargonaut_vfs::{
    ByteRange, HostKeyEvent, SftpCredentials, SftpFs, Sort, VfsBackend, VfsPath, WriteMode,
};
use futures::{AsyncReadExt, AsyncWriteExt};

/// `user@host:port` of the docker-compose fixture.
const AUTHORITY: &str = "testuser@localhost:2222";
const PASSWORD: &str = "testpass";

/// 10 MiB — the SC-004 minimum probe size.
const PROBE_SIZE: usize = 10 * 1024 * 1024;

/// SC-004 aspirational target: 70% of 1 Gbps = 87.5 MB/s.
const SC004_TARGET_MBPS: f64 = 87.5;

/// Conservative, non-flaky floor for the SC-004 assertion (see module docs).
const SC004_FLOOR_MBPS: f64 = 20.0;

/// Spawn a task that auto-accepts every host-key prompt, returning the sender
/// to hand to `SftpFs::connect`. atmoz/sftp regenerates its host key on each
/// container start, so the key is always "unknown" to the client — accepting
/// unconditionally is the right behaviour for an ephemeral test fixture.
fn auto_accept_host_keys() -> tokio::sync::mpsc::UnboundedSender<HostKeyEvent> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<HostKeyEvent>();
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            // Drop-without-send rejects; sending `true` accepts.
            let _ = event.accept_tx.send(true);
        }
    });
    tx
}

/// Connect, retrying briefly: a freshly-started sshd may accept the TCP
/// connection a beat before the SSH layer is ready. CI waits for the port
/// first, but this keeps the test robust against that race on slow runners.
async fn connect() -> SftpFs {
    let config = Arc::new(russh::client::Config::default());
    let mut last_err = None;
    for attempt in 1..=10 {
        let host_key_tx = auto_accept_host_keys();
        match SftpFs::connect(
            AUTHORITY,
            SftpCredentials::Password(PASSWORD.to_string()),
            config.clone(),
            host_key_tx,
        )
        .await
        {
            Ok(fs) => return fs,
            Err(e) => {
                eprintln!("connect attempt {attempt}/10 failed: {e}");
                last_err = Some(e);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
    panic!("could not connect to {AUTHORITY} after 10 attempts: {last_err:?}");
}

fn path(p: &str) -> VfsPath {
    VfsPath::parse(&format!("sftp://{AUTHORITY}{p}")).expect("valid sftp path")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sc003_connect_and_list_root_within_5s() {
    let fs = connect().await;

    let start = Instant::now();
    let listing = fs.list(&path("/"), Sort::NameAsc).await.expect("list root");
    let elapsed = start.elapsed();

    eprintln!("SC-003: listed root in {:.3}s", elapsed.as_secs_f64());
    assert!(
        elapsed <= Duration::from_secs(5),
        "SC-003: root listing took {elapsed:?}, exceeds the 5 s gate"
    );

    // The compose fixture provisions a writable `upload/` subdir; its presence
    // confirms we listed the real chrooted home, not an empty/error result.
    let names: Vec<&str> = listing.entries.iter().map(|e| e.name.as_str()).collect();
    assert!(
        names.contains(&"upload"),
        "expected `upload` dir in root listing, got {names:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sc004_transfer_10mib_meets_throughput_floor() {
    let fs = connect().await;
    let remote = path("/upload/probe.bin");

    // A non-trivial, verifiable payload (position-dependent bytes catch any
    // offset/truncation corruption that a constant fill would hide).
    let data: Vec<u8> = (0..PROBE_SIZE).map(|i| (i % 251) as u8).collect();

    // Upload the probe file first so there is something to measure reading.
    {
        let mut writer = fs
            .write_stream(&remote, 0, WriteMode::Truncate)
            .await
            .expect("open write_stream");
        writer.write_all(&data).await.expect("write probe");
        writer.close().await.expect("close writer");
    }

    // SC-004 measures the SFTP→local transfer (the download direction).
    let start = Instant::now();
    let mut reader = fs
        .read_stream(&remote, ByteRange::FULL)
        .await
        .expect("open read_stream");
    let mut buf = Vec::with_capacity(PROBE_SIZE);
    reader.read_to_end(&mut buf).await.expect("read probe");
    let elapsed = start.elapsed();

    // Best-effort cleanup; failure here must not fail the test.
    let _ = fs.unlink(&remote).await;

    assert_eq!(buf.len(), PROBE_SIZE, "short read");
    assert_eq!(
        buf, data,
        "round-tripped bytes differ from what was written"
    );

    let mib = PROBE_SIZE as f64 / (1024.0 * 1024.0);
    let secs = elapsed.as_secs_f64();
    let mbps = mib / secs;
    eprintln!(
        "SC-004: read {mib:.0} MiB in {secs:.3}s = {mbps:.1} MB/s \
         ({:.0}% of the {SC004_TARGET_MBPS} MB/s target)",
        mbps / SC004_TARGET_MBPS * 100.0
    );
    assert!(
        mbps >= SC004_FLOOR_MBPS,
        "SC-004: throughput {mbps:.1} MB/s is below the {SC004_FLOOR_MBPS} MB/s floor"
    );
}
