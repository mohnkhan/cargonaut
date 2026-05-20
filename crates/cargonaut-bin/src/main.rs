// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Cargonaut binary — argument parsing, config load, App boot, signal handlers.
//!
//! Phase 1 prototype: this is a runnable stub that prints the loaded
//! config + a "hello, world" message. T1.21 wires real terminal init and
//! event loop. T1.07+T1.17+T1.18+T1.19 fill in the UI.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Cargonaut — Rust-native dual-pane terminal file manager.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Path for the LEFT pane (default: $HOME)
    left: Option<PathBuf>,
    /// Path for the RIGHT pane (default: /tmp)
    right: Option<PathBuf>,

    /// Path to alternate config file.
    #[arg(long)]
    config: Option<PathBuf>,

    /// Override theme.
    #[arg(long)]
    theme: Option<String>,

    /// Load MC-compat keymap.
    #[arg(long)]
    mc_keys: bool,

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
    /// List installed plugins + their granted capabilities.
    ListPlugins,
    /// Dump or rotate the audit log.
    Audit {
        #[arg(long)]
        rotate: bool,
    },
    /// List resumable transfers; optionally resume one.
    Resume {
        #[arg(long)]
        id: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    let _config = cargonaut_config::Config::default(); // T1.16 → Config::load(cli.config)

    match cli.cmd {
        Some(CargonautCommand::ListPlugins) => {
            println!("Plugins: (none — Phase 3)");
            return Ok(());
        }
        Some(CargonautCommand::Audit { rotate: _ }) => {
            println!("Audit: (not yet — Phase 4)");
            return Ok(());
        }
        Some(CargonautCommand::Resume { id: _ }) => {
            println!("Resume: (not yet — Phase 1 T1.14)");
            return Ok(());
        }
        None => {}
    }

    let left = cli
        .left
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| "/".into()));
    let right = cli.right.unwrap_or_else(|| "/tmp".into());

    println!(
        "cargonaut {} — Phase 1 prototype stub",
        env!("CARGO_PKG_VERSION")
    );
    println!("  left pane:  {}", left.display());
    println!("  right pane: {}", right.display());
    println!(
        "  theme:      {}",
        cli.theme.unwrap_or_else(|| "solarized-dark".into())
    );
    println!();
    println!("UI not yet wired (see design/tasks.md T1.07+).");

    // T1.19+T1.21: build App, run TUI event loop:
    // let app = cargonaut_core::App::new(_config, &left.to_string_lossy(), &right.to_string_lossy());
    // cargonaut_ui_tui::run(app).await?;
    Ok(())
}

fn init_tracing(verbose: bool) {
    let filter = if verbose { "debug" } else { "info" };
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| filter.into()),
        )
        .with_writer(std::io::stderr)
        .try_init();
}

// `dirs` crate is a small dep we'll add when we wire real home detection;
// for the stub, use a fallback.
mod dirs {
    pub fn home_dir() -> Option<std::path::PathBuf> {
        std::env::var_os("HOME").map(Into::into)
    }
}
