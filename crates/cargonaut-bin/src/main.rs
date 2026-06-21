// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Cargonaut binary — clap CLI + config load + App boot + TUI launch.
//!
//! Phase 1: launches the dual-pane TUI for two given paths (defaulting
//! to `$HOME` and `/tmp`). Subcommands (`list-plugins`, `audit`,
//! `resume`) stub features that land in later phases.

use cargonaut_core::diag;
use clap::{Parser, Subcommand};
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;

use futures::FutureExt;

/// Long `--version` output: version + copyright + license + repo (Feature 061,
/// FR-011). Built from the same identity as `diag::about_lines()`.
const LONG_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "\n© 2024–2026 Mohiuddin Khan Inamdar",
    "\nLicense: MIT OR Apache-2.0",
    "\nhttps://github.com/mohnkhan/cargonaut",
);

/// Cargonaut — Rust-native dual-pane terminal file manager.
#[derive(Parser, Debug)]
#[command(version, long_version = LONG_VERSION, about, long_about = None)]
struct Cli {
    /// Path for the LEFT pane (default: $HOME).
    left: Option<PathBuf>,
    /// Path for the RIGHT pane (default: /tmp).
    right: Option<PathBuf>,

    /// Path to alternate config file.
    #[arg(long)]
    config: Option<PathBuf>,

    /// Override theme.
    #[arg(long)]
    theme: Option<String>,

    /// Load the orthodox-FM-compat keymap.
    #[arg(long)]
    mc_keys: bool,

    /// Disable mouse capture for this session (mouse is on by default).
    #[arg(long)]
    no_mouse: bool,

    /// Enable a plugin for this session only (repeatable).
    #[arg(long)]
    enable_plugin: Vec<String>,

    /// Emit a plain-text event stream for screen readers.
    #[arg(long, value_name = "MODE")]
    a11y_output: Option<String>,

    /// Debug logging to stderr.
    #[arg(short, long)]
    verbose: bool,

    #[command(subcommand)]
    cmd: Option<CargonautCommand>,
}

#[derive(Subcommand, Debug)]
enum CargonautCommand {
    /// List installed plugins + their granted capabilities (Phase 3).
    ListPlugins,
    /// Dump or rotate the audit log (Phase 4).
    Audit {
        /// Rotate the audit log instead of dumping it.
        #[arg(long)]
        rotate: bool,
    },
    /// List resumable transfers; optionally resume one by id.
    Resume {
        /// Specific transfer id to resume.
        #[arg(long)]
        id: Option<String>,
    },
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    // Feature 061 (US1): install the capture hook before anything else so every
    // subsequent panic is recorded (and the default stderr dump suppressed).
    diag::install_panic_hook();

    let cli = Cli::parse();
    init_tracing(cli.verbose);

    // Feature 061 (US3 / FR-006a): one-time notice if a prior crash report is
    // unseen. Best-effort; never blocks startup.
    notify_unseen_crash();

    let mut config = match cli.config.as_deref() {
        Some(path) => cargonaut_config::Config::load_from_path(path)?,
        None => cargonaut_config::Config::load().unwrap_or_default(),
    };

    // Feature 031 (FR-005): apply CLI overrides that were previously
    // parsed but dropped. `--theme` now takes effect; `--mc-keys` and
    // `--no-mouse` likewise merge into the effective config.
    if let Some(theme) = cli.theme.clone() {
        config.ui.theme = theme;
    }
    if cli.mc_keys {
        config.ui.mc_keys = true;
    }
    if cli.no_mouse {
        config.ui.mouse = false;
    }

    if let Some(sub) = cli.cmd {
        return run_subcommand(sub).await;
    }

    let left = path_arg(cli.left.clone(), home_dir());
    let right = path_arg(cli.right.clone(), "/tmp".into());

    // Feature 061 (US1): the whole app session runs inside an outer catch_unwind
    // so a panic during startup (before the UI's own boundary) is also caught and
    // turned into a clean crash report instead of an unwinding-out-of-main dump.
    let outcome = AssertUnwindSafe(async {
        diag::maybe_inject_panic("startup");
        let mut app =
            cargonaut_core::App::new(config, &left.to_string_lossy(), &right.to_string_lossy())
                .await
                .map_err(|e| cargonaut_ui_tui::Error::Other(e.to_string()))?;

        let run_result = cargonaut_ui_tui::run(&mut app).await;

        // FR-017 exit-cwd writer: when invoked via the contrib/cargonaut.sh
        // wrapper (which sets $CARGONAUT_EXIT_CWD_FILE), write the active pane's
        // cwd so the wrapper can `cd` to it after exit. Best-effort.
        if let Ok(path) = std::env::var("CARGONAUT_EXIT_CWD_FILE") {
            if !path.is_empty() {
                let cwd = active_pane_local_path(&app);
                if let Err(e) = std::fs::write(&path, cwd.as_bytes()) {
                    tracing::warn!("could not write exit-cwd file {path}: {e}");
                }
            }
        }
        run_result
    })
    .catch_unwind()
    .await;

    match outcome {
        Ok(Ok(())) => Ok(()),
        // The UI's outer boundary caught a panic, restored the terminal, and
        // returned FatalPanic. Write the report (FR-002/006) and exit non-zero.
        Ok(Err(cargonaut_ui_tui::Error::FatalPanic)) => {
            finish_with_crash_report();
            std::process::exit(101);
        }
        // A normal (non-panic) error from startup or the loop.
        Ok(Err(other)) => Err(other.into()),
        // A panic escaped the async body (e.g. during startup, before the UI
        // boundary existed). The terminal was never entered, so just report.
        Err(_payload) => {
            finish_with_crash_report();
            std::process::exit(101);
        }
    }
}

/// Format + persist the captured panic as a crash report and tell the user where
/// it went. Failure-tolerant (FR-013): on any IO error the user is told the
/// report could not be saved, never a secondary panic. Output goes to stderr as
/// plain text — safe for a non-TTY / a11y stream (FR-016).
fn finish_with_crash_report() {
    let dir = diag::data_dir();
    let message = match diag::take_captured_panic() {
        Some(cap) => {
            let meta = diag::ReportMeta::current(diag::timestamp_utc());
            let body = diag::format_crash_report(&meta, &cap);
            match diag::write_report(&dir, &meta.timestamp, &body) {
                Ok(path) => {
                    let _ = diag::prune_reports(&dir, diag::DEFAULT_RETENTION);
                    format!(
                        "cargonaut crashed. Crash report saved to: {}",
                        path.display()
                    )
                }
                Err(e) => format!("cargonaut crashed. Could not save crash report: {e}"),
            }
        }
        None => "cargonaut crashed (no panic details were captured).".to_string(),
    };
    eprintln!("{message}");
}

/// Feature 061 (FR-006a): if a crash report exists that the user hasn't been
/// shown yet, print a one-time pointer to it and mark it seen. Best-effort.
fn notify_unseen_crash() {
    let dir = diag::data_dir();
    if let Ok(Some(path)) = diag::unseen_report(&dir) {
        eprintln!(
            "note: a previous cargonaut session crashed — report at {}",
            path.display()
        );
        let _ = diag::mark_seen(&dir, &path);
    }
}

/// Active pane's cwd as a local-filesystem path string (stripped of the
/// `file://` scheme). Phase 1 LocalFs only; non-`file://` panes return
/// the raw `VfsPath::display()` so the wrapper script's `cd` fails
/// loudly rather than silently jumping to the wrong dir.
fn active_pane_local_path(app: &cargonaut_core::App) -> String {
    let display = app.active_pane_state().cwd.display();
    display
        .strip_prefix("file://")
        .unwrap_or(&display)
        .to_string()
}

fn path_arg(p: Option<PathBuf>, default: PathBuf) -> PathBuf {
    match p {
        Some(p) if p.is_absolute() => p,
        Some(p) => std::env::current_dir().unwrap_or_default().join(p),
        None => default,
    }
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

async fn run_subcommand(sub: CargonautCommand) -> anyhow::Result<()> {
    match sub {
        CargonautCommand::ListPlugins => {
            println!("Plugins: (none — Phase 3)");
        }
        CargonautCommand::Audit { rotate } => {
            if rotate {
                println!("Audit log rotation not implemented (Phase 4 — T4.x)");
            } else {
                println!("Audit log dump not implemented (Phase 4 — T4.x)");
            }
        }
        CargonautCommand::Resume { id } => match id {
            Some(id) => println!("Resume {id}: not yet implemented (Phase 1 polish)"),
            None => println!("Resume listing: not yet implemented (Phase 1 polish)"),
        },
    }
    Ok(())
}

fn init_tracing(verbose: bool) {
    let default = if verbose { "debug" } else { "info" };

    // Stderr layer — shown only if verbose
    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| default.into()),
        );

    // File layer — always write WARN+ to ~/.local/share/cargonaut/debug.log
    let file_layer = {
        let log_dir = std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
            .join(".local/share/cargonaut");
        let _ = std::fs::create_dir_all(&log_dir);
        let log_path = log_dir.join("debug.log");
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            Ok(file) => Some(
                tracing_subscriber::fmt::layer()
                    .with_writer(std::sync::Arc::new(file))
                    .with_ansi(false)
                    .with_filter(tracing_subscriber::filter::LevelFilter::WARN),
            ),
            Err(_) => None,
        }
    };

    use tracing_subscriber::prelude::*;
    let _ = tracing_subscriber::registry()
        .with(stderr_layer)
        .with(file_layer)
        .try_init();
}
