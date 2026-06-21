// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Diagnostics & survivability support (Feature 061).
//!
//! This module is the secret-free, mostly-pure core of the crash-safety work:
//!
//! - a process-global **recent-action ring buffer** ([`record_action`],
//!   [`recent_actions`]) so a crash report can show the lead-up the WARN-only
//!   `debug.log` cannot;
//! - a global **panic-capture** seam ([`install_panic_hook`],
//!   [`take_captured_panic`]) that records message/location/thread/backtrace +
//!   an action snapshot *without* touching the terminal or writing files (safe
//!   to fire on any thread);
//! - a deterministic, IO-free **crash-report formatter** ([`format_crash_report`]);
//! - failure-tolerant **crash-file lifecycle** ([`write_report`],
//!   [`prune_reports`], [`unseen_report`], [`mark_seen`]);
//! - the single-source **About** identity ([`about`], [`about_lines`]);
//! - a test-only fault injector ([`maybe_inject_panic`]).
//!
//! See `specs/061-survivability-and-about/contracts/diag-api.md`.

use std::backtrace::Backtrace;
use std::collections::VecDeque;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, Once, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Maximum entries retained in the recent-action ring buffer.
pub const ACTION_CAPACITY: usize = 64;

/// Default number of crash reports kept on disk by [`prune_reports`].
pub const DEFAULT_RETENTION: usize = 10;

// ─── Recent-action ring buffer ───────────────────────────────────────────────

/// One entry in the recent-action trail. Constructed only by [`record_action`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionRecord {
    /// Monotonic sequence number (process-global).
    pub seq: u64,
    /// Command/event variant name, e.g. `"CursorDown"`. Stable identifier.
    pub label: String,
    /// Coarse, **secret-free** context (e.g. `"pane=Left idx=12"`); never raw
    /// paths' contents, free text, or credentials.
    pub detail: Option<String>,
}

fn actions() -> &'static Mutex<VecDeque<ActionRecord>> {
    static ACTIONS: OnceLock<Mutex<VecDeque<ActionRecord>>> = OnceLock::new();
    ACTIONS.get_or_init(|| Mutex::new(VecDeque::with_capacity(ACTION_CAPACITY)))
}

static NEXT_SEQ: AtomicU64 = AtomicU64::new(1);

/// Lock a mutex, recovering from poisoning. Diagnostics must never wedge just
/// because a thread panicked while briefly holding a diag lock.
fn lock_recover<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Record one user action/command into the process-global ring buffer.
///
/// `detail` MUST be secret-free (pane id / index only — no paths' contents or
/// credentials). O(1); oldest entry dropped past [`ACTION_CAPACITY`].
pub fn record_action(label: &str, detail: Option<&str>) {
    let seq = NEXT_SEQ.fetch_add(1, Ordering::Relaxed);
    let mut buf = lock_recover(actions());
    buf.push_back(ActionRecord {
        seq,
        label: label.to_string(),
        detail: detail.map(str::to_string),
    });
    while buf.len() > ACTION_CAPACITY {
        buf.pop_front();
    }
}

/// Snapshot of the recent-action trail, oldest first.
pub fn recent_actions() -> Vec<ActionRecord> {
    lock_recover(actions()).iter().cloned().collect()
}

#[cfg(test)]
fn clear_actions() {
    lock_recover(actions()).clear();
}

// ─── Panic capture ───────────────────────────────────────────────────────────

/// Structured snapshot of a panic, filled by the global hook and consumed by a
/// catch site (recoverable site discards it; the fatal site formats a report).
#[derive(Debug, Clone)]
pub struct CapturedPanic {
    /// The panic payload as a string (best-effort downcast).
    pub message: String,
    /// `file:line:col` of the panic, when the runtime provided one.
    pub location: Option<String>,
    /// Name of the thread that panicked (`"unnamed"` if none).
    pub thread: String,
    /// Backtrace captured at panic time (`force_capture`, env-independent).
    pub backtrace: String,
    /// Recent-action trail at the moment of the panic.
    pub actions: Vec<ActionRecord>,
}

fn last_panic() -> &'static Mutex<Option<CapturedPanic>> {
    static LAST_PANIC: OnceLock<Mutex<Option<CapturedPanic>>> = OnceLock::new();
    LAST_PANIC.get_or_init(|| Mutex::new(None))
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

/// Install the global panic hook (idempotent).
///
/// The hook captures message/location/thread + [`Backtrace::force_capture`] + a
/// recent-action snapshot into a global slot and emits a `tracing::error!`. It
/// deliberately does **not** touch the terminal or write any file, so it is safe
/// to fire on a background worker thread without disturbing a live UI. The
/// default stderr panic dump is replaced (it would corrupt the alternate screen).
pub fn install_panic_hook() {
    static HOOK_ONCE: Once = Once::new();
    HOOK_ONCE.call_once(|| {
        std::panic::set_hook(Box::new(|info| {
            let captured = CapturedPanic {
                message: panic_message(info.payload()),
                location: info
                    .location()
                    .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column())),
                thread: std::thread::current()
                    .name()
                    .unwrap_or("unnamed")
                    .to_string(),
                backtrace: Backtrace::force_capture().to_string(),
                actions: recent_actions(),
            };
            tracing::error!(
                target: "cargonaut::panic",
                message = %captured.message,
                location = ?captured.location,
                thread = %captured.thread,
                "panic captured"
            );
            *lock_recover(last_panic()) = Some(captured);
        }));
    });
}

/// Take the most recently captured panic, clearing the slot. A catch site calls
/// this after `catch_unwind` returns `Err` to obtain the rich details.
pub fn take_captured_panic() -> Option<CapturedPanic> {
    lock_recover(last_panic()).take()
}

// ─── Test-only fault injection ───────────────────────────────────────────────

/// Panic once if `CARGONAUT_PANIC_INJECT` equals `site`. Inert when the variable
/// is unset (normal operation). Sites: `"startup" | "render" | "input" | "task"`.
pub fn maybe_inject_panic(site: &str) {
    if std::env::var("CARGONAUT_PANIC_INJECT").ok().as_deref() == Some(site) {
        panic!("injected panic at site: {site}");
    }
}

// ─── Data directory + timestamp ──────────────────────────────────────────────

/// Resolve the per-user data dir: `$XDG_DATA_HOME/cargonaut` else
/// `$HOME/.local/share/cargonaut` else `/tmp/cargonaut`. Does not create it.
pub fn data_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME").filter(|s| !s.is_empty()) {
        return PathBuf::from(xdg).join("cargonaut");
    }
    if let Some(home) = std::env::var_os("HOME").filter(|s| !s.is_empty()) {
        return PathBuf::from(home).join(".local/share/cargonaut");
    }
    PathBuf::from("/tmp/cargonaut")
}

/// Current UTC timestamp as `YYYYMMDD-HHMMSSmmm` (lexicographic == chronological).
/// Dependency-free; uses the civil-from-days algorithm.
pub fn timestamp_utc() -> String {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format_epoch_millis(dur.as_millis() as u64)
}

fn format_epoch_millis(ms: u64) -> String {
    let secs = ms / 1000;
    let millis = ms % 1000;
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, mo, d) = civil_from_days(days);
    format!("{y:04}{mo:02}{d:02}-{h:02}{mi:02}{s:02}{millis:03}")
}

/// Convert days-since-Unix-epoch to (year, month, day) UTC (Howard Hinnant).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

// ─── Crash-report formatter (pure) ───────────────────────────────────────────

/// Deterministic inputs for [`format_crash_report`].
#[derive(Debug, Clone)]
pub struct ReportMeta {
    /// Application version (`CARGO_PKG_VERSION`).
    pub app_version: &'static str,
    /// Operating system (`std::env::consts::OS`).
    pub os: &'static str,
    /// CPU architecture (`std::env::consts::ARCH`).
    pub arch: &'static str,
    /// UTC timestamp string (see [`timestamp_utc`]).
    pub timestamp: String,
}

impl ReportMeta {
    /// Build a [`ReportMeta`] for the running build at the given timestamp.
    pub fn current(timestamp: String) -> Self {
        Self {
            app_version: env!("CARGO_PKG_VERSION"),
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            timestamp,
        }
    }
}

/// Format a self-contained, human-readable crash report. IO-free and
/// deterministic. Guaranteed to contain version, platform, location (if any) and
/// backtrace; built only from secret-free inputs (FR-015).
pub fn format_crash_report(meta: &ReportMeta, panic: &CapturedPanic) -> String {
    let mut out = String::with_capacity(1024 + panic.backtrace.len());
    out.push_str("cargonaut crash report\n");
    out.push_str("=======================\n");
    out.push_str(&format!("version:   {}\n", meta.app_version));
    out.push_str(&format!("when:      {} UTC\n", meta.timestamp));
    out.push_str(&format!("platform:  {}/{}\n", meta.os, meta.arch));
    out.push_str(&format!("thread:    {}\n\n", panic.thread));

    out.push_str("## Panic\n");
    out.push_str(&format!("message:   {}\n", panic.message));
    out.push_str(&format!(
        "location:  {}\n\n",
        panic.location.as_deref().unwrap_or("<unknown>")
    ));

    out.push_str("## Recent actions (oldest first)\n");
    if panic.actions.is_empty() {
        out.push_str("  (none recorded)\n");
    } else {
        for (i, a) in panic.actions.iter().enumerate() {
            match &a.detail {
                Some(d) => out.push_str(&format!("  {:>3}. {:<20} {}\n", i + 1, a.label, d)),
                None => out.push_str(&format!("  {:>3}. {}\n", i + 1, a.label)),
            }
        }
    }
    out.push('\n');

    out.push_str("## Backtrace\n");
    out.push_str(&panic.backtrace);
    if !panic.backtrace.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(
        "(note: release builds strip symbols; frames may be partial — message + location above are authoritative)\n",
    );
    out
}

// ─── Crash-file lifecycle (failure-tolerant IO) ──────────────────────────────

/// Write `crash-<timestamp>.log` under `dir`, creating `dir` if needed.
pub fn write_report(dir: &Path, timestamp: &str, body: &str) -> io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("crash-{timestamp}.log"));
    std::fs::write(&path, body)?;
    Ok(path)
}

fn report_names(dir: &Path) -> io::Result<Vec<String>> {
    let mut names: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let name = entry?.file_name().to_string_lossy().into_owned();
        if name.starts_with("crash-") && name.ends_with(".log") {
            names.push(name);
        }
    }
    names.sort(); // lexical == chronological
    Ok(names)
}

/// Keep the newest `keep` `crash-*.log` files in `dir`, deleting older ones.
/// `keep == 0` is treated as 1 (never prune everything by accident).
pub fn prune_reports(dir: &Path, keep: usize) -> io::Result<()> {
    let keep = keep.max(1);
    let names = report_names(dir)?;
    if names.len() <= keep {
        return Ok(());
    }
    for name in &names[..names.len() - keep] {
        let _ = std::fs::remove_file(dir.join(name)); // best-effort
    }
    Ok(())
}

/// Newest `crash-*.log` not yet acknowledged via [`mark_seen`], else `None`.
pub fn unseen_report(dir: &Path) -> io::Result<Option<PathBuf>> {
    let names = match report_names(dir) {
        Ok(n) => n,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let Some(newest) = names.last() else {
        return Ok(None);
    };
    let seen = std::fs::read_to_string(dir.join("crash-seen"))
        .ok()
        .map(|s| s.trim().to_string());
    match seen {
        Some(seen) if *newest <= seen => Ok(None),
        _ => Ok(Some(dir.join(newest))),
    }
}

/// Record `report` as acknowledged so [`unseen_report`] won't return it again.
pub fn mark_seen(dir: &Path, report: &Path) -> io::Result<()> {
    let name = report
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("crash-seen"), name)
}

// ─── About identity ──────────────────────────────────────────────────────────

/// Static build identity, the single source for every About surface.
#[derive(Debug, Clone, Copy)]
pub struct AboutInfo {
    /// Application name.
    pub name: &'static str,
    /// Version (`CARGO_PKG_VERSION`).
    pub version: &'static str,
    /// Author.
    pub author: &'static str,
    /// Copyright notice.
    pub copyright: &'static str,
    /// SPDX license identifier.
    pub license: &'static str,
    /// Project repository URL.
    pub repository: &'static str,
}

/// The build identity for this binary.
pub fn about() -> AboutInfo {
    AboutInfo {
        name: "cargonaut",
        version: env!("CARGO_PKG_VERSION"),
        author: "Mohiuddin Khan Inamdar",
        copyright: "© 2024–2026 Mohiuddin Khan Inamdar",
        license: "MIT OR Apache-2.0",
        repository: "https://github.com/mohnkhan/cargonaut",
    }
}

/// Rendered About lines shared by the F1 Help section, the About dialog, and
/// (joined) the CLI long-version output. Contains name, version, author,
/// copyright, and license (SC-005).
pub fn about_lines() -> Vec<String> {
    let a = about();
    vec![
        format!("{} {} — dual-pane terminal file manager", a.name, a.version),
        format!("Author:  {}", a.author),
        a.copyright.to_string(),
        format!("License: {}", a.license),
        a.repository.to_string(),
    ]
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Serializes tests that touch the process-global ring buffer / panic slot.
    static SERIAL: Mutex<()> = Mutex::new(());

    fn sample_panic() -> CapturedPanic {
        CapturedPanic {
            message: "index out of bounds: the len is 3 but the index is 7".into(),
            location: Some("crates/cargonaut-ui-tui/src/pane.rs:412:18".into()),
            thread: "main".into(),
            backtrace: "   0: some::frame\n   1: another::frame".into(),
            actions: vec![
                ActionRecord {
                    seq: 1,
                    label: "FocusSwap".into(),
                    detail: None,
                },
                ActionRecord {
                    seq: 2,
                    label: "CursorDown".into(),
                    detail: Some("pane=Left idx=11".into()),
                },
            ],
        }
    }

    #[test]
    fn action_buffer_caps_at_capacity_and_keeps_newest() {
        let _g = SERIAL.lock().unwrap();
        clear_actions();
        for i in 0..(ACTION_CAPACITY + 6) {
            record_action(&format!("Cmd{i}"), None);
        }
        let snap = recent_actions();
        assert_eq!(snap.len(), ACTION_CAPACITY, "buffer must cap at capacity");
        // Oldest 6 dropped → first remaining is Cmd6, last is Cmd{cap+5}.
        assert_eq!(snap.first().unwrap().label, "Cmd6");
        assert_eq!(
            snap.last().unwrap().label,
            format!("Cmd{}", ACTION_CAPACITY + 5)
        );
        // seq strictly increases.
        assert!(snap.windows(2).all(|w| w[1].seq > w[0].seq));
    }

    #[test]
    fn capture_slot_take_is_none_then_roundtrips() {
        let _g = SERIAL.lock().unwrap();
        // Drain any residue from earlier tests in this binary.
        let _ = take_captured_panic();
        assert!(take_captured_panic().is_none());

        install_panic_hook();
        let res = std::panic::catch_unwind(|| panic!("boom-xyz"));
        assert!(res.is_err());
        let cap = take_captured_panic().expect("hook must populate the slot");
        assert!(cap.message.contains("boom-xyz"));
        assert!(cap.location.is_some(), "location must be captured");
        assert!(!cap.backtrace.is_empty(), "backtrace must be captured");
        // Slot cleared after take.
        assert!(take_captured_panic().is_none());
    }

    #[test]
    fn maybe_inject_panic_is_inert_when_unset() {
        // CARGONAUT_PANIC_INJECT is not set in the test environment.
        maybe_inject_panic("render"); // must not panic
    }

    #[test]
    fn format_crash_report_is_deterministic_and_complete() {
        let meta = ReportMeta {
            app_version: "9.9.9",
            os: "linux",
            arch: "x86_64",
            timestamp: "20260621-143022417".into(),
        };
        let p = sample_panic();
        let a = format_crash_report(&meta, &p);
        let b = format_crash_report(&meta, &p);
        assert_eq!(a, b, "must be deterministic");
        for needle in [
            "version:   9.9.9",
            "platform:  linux/x86_64",
            "## Panic",
            "location:  crates/cargonaut-ui-tui/src/pane.rs:412:18",
            "## Recent actions",
            "FocusSwap",
            "## Backtrace",
        ] {
            assert!(a.contains(needle), "report missing {needle:?}\n{a}");
        }
    }

    #[test]
    fn report_omits_a_configured_secret() {
        // SC-008: a secret never enters the report because it is never collected
        // into the action trail or any captured field.
        let meta = ReportMeta::current("20260101-000000000".into());
        let p = CapturedPanic {
            message: "boom".into(),
            location: None,
            thread: "main".into(),
            backtrace: "frame".into(),
            actions: vec![ActionRecord {
                seq: 1,
                label: "SftpConnect".into(),
                detail: Some("host=example.com user=alice".into()), // no password
            }],
        };
        let report = format_crash_report(&meta, &p);
        assert!(!report.contains("hunter2"), "secret must never appear");
    }

    #[test]
    fn write_prune_and_unseen_lifecycle() {
        let dir = std::env::temp_dir().join(format!("cargonaut-diag-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        // Write 12 reports; prune keeps newest 10.
        let mut last = PathBuf::new();
        for i in 0..12 {
            let ts = format!("20260101-0000000{i:02}"); // lexical order
            last = write_report(&dir, &ts, "body").unwrap();
        }
        prune_reports(&dir, DEFAULT_RETENTION).unwrap();
        let remaining = report_names(&dir).unwrap();
        assert_eq!(remaining.len(), DEFAULT_RETENTION);
        assert!(
            remaining.last().unwrap().ends_with("000000011.log"),
            "newest kept"
        );
        assert!(
            remaining.first().unwrap().ends_with("000000002.log"),
            "kept window starts at i=2"
        );
        assert!(
            !remaining
                .iter()
                .any(|n| n.ends_with("000000000.log") || n.ends_with("000000001.log")),
            "the two oldest reports must be pruned"
        );

        // Next-launch notice fires once, then not again (SC-009).
        let unseen = unseen_report(&dir).unwrap();
        assert_eq!(unseen.as_deref(), Some(last.as_path()));
        mark_seen(&dir, &last).unwrap();
        assert!(
            unseen_report(&dir).unwrap().is_none(),
            "must not repeat once seen"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unseen_report_none_when_dir_absent() {
        let dir = std::env::temp_dir().join("cargonaut-diag-absent-xyz");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(unseen_report(&dir).unwrap().is_none());
    }

    #[test]
    fn about_lines_carry_identity() {
        let lines = about_lines().join("\n");
        assert!(lines.contains(env!("CARGO_PKG_VERSION")), "version present");
        assert!(lines.contains("Mohiuddin Khan Inamdar"), "author present");
        assert!(lines.contains("©"), "copyright present");
        assert!(lines.contains("MIT OR Apache-2.0"), "license present");
    }

    #[test]
    fn timestamp_format_shape_and_known_epoch() {
        let ts = timestamp_utc();
        assert_eq!(ts.len(), 18, "YYYYMMDD-HHMMSSmmm = 18 chars, got {ts:?}");
        assert_eq!(ts.as_bytes()[8], b'-');
        // 2021-01-01T00:00:00.000Z = 1609459200000 ms.
        assert_eq!(format_epoch_millis(1_609_459_200_000), "20210101-000000000");
    }
}
