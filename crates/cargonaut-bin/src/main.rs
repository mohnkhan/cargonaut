// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Cargonaut binary — clap CLI + config load + App boot + TUI launch.
//!
//! Phase 1: launches the dual-pane TUI for two given paths (defaulting
//! to `$HOME` and `/tmp`). Subcommands (`list-plugins`, `audit`,
//! `resume`) stub features that land in later phases.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Cargonaut — Rust-native dual-pane terminal file manager.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
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
    let cli = Cli::parse();
    init_tracing(cli.verbose);

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

    let mut app =
        cargonaut_core::App::new(config, &left.to_string_lossy(), &right.to_string_lossy()).await?;

    let run_result = cargonaut_ui_tui::run(&mut app).await;

    // FR-017 exit-cwd writer: when invoked via the contrib/cargonaut.sh
    // wrapper (which sets $CARGONAUT_EXIT_CWD_FILE), write the active
    // pane's cwd to that file so the wrapper can `cd` to it after exit.
    // Best-effort: silent on missing var; logs on write failure.
    if let Ok(path) = std::env::var("CARGONAUT_EXIT_CWD_FILE") {
        if !path.is_empty() {
            let cwd = active_pane_local_path(&app);
            if let Err(e) = std::fs::write(&path, cwd.as_bytes()) {
                tracing::warn!("could not write exit-cwd file {path}: {e}");
            }
        }
    }

    run_result?;
    Ok(())
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
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| default.into()),
        )
        .with_writer(std::io::stderr)
        .try_init();
}
