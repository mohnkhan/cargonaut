// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0
//
// SC-005 bench: skin file loaded and applied in under 20 ms on cold startup.
// Exercises Theme::resolve (builtin path) and load_skin (TOML parse path).
//
// Run with:  cargo bench -p cargonaut-ui-tui --bench theme_resolve

use cargonaut_ui_tui::theme::{load_skin, Theme};
use std::fs;
use std::time::Instant;

const ITERS: u32 = 10_000;
const DRACULA_TOML: &str = r##"
panel_bg  = "#282a36"
panel_fg  = "#f8f8f2"
dir_fg    = "#8be9fd"
exec_fg   = "#50fa7b"
symlink_fg = "#ff79c6"
hidden_fg  = "#6272a4"
cursor_bg  = "#ff79c6"
cursor_fg  = "#282a36"
marked_fg  = "#f1fa8c"
border_focused   = "#ff79c6"
border_unfocused = "#6272a4"
menu_bg    = "#44475a"
menu_fg    = "#f8f8f2"
menu_sel_bg = "#6272a4"
menu_sel_fg = "#f8f8f2"
fkey_num_bg  = "#44475a"
fkey_num_fg  = "#ff79c6"
fkey_label_bg = "#282a36"
fkey_label_fg = "#6272a4"
status_bg  = "#44475a"
status_fg  = "#f8f8f2"
dialog_bg  = "#44475a"
dialog_fg  = "#f8f8f2"
dialog_sel_bg = "#6272a4"
dialog_sel_fg = "#f8f8f2"
"##;

fn main() {
    // --- builtin resolve ---
    let t0 = Instant::now();
    for _ in 0..ITERS {
        let _ = std::hint::black_box(Theme::resolve("commander-dark"));
    }
    let builtin_ns = t0.elapsed().as_nanos() / u128::from(ITERS);
    println!("Theme::resolve builtin: {builtin_ns} ns/iter  (budget: 20_000_000 ns)");

    // --- full skin file load (from filesystem) ---
    let dir = tempfile::TempDir::new().expect("tempdir");
    let themes = dir.path().join("themes");
    fs::create_dir_all(&themes).unwrap();
    fs::write(themes.join("dracula.toml"), DRACULA_TOML).unwrap();

    let t1 = Instant::now();
    for _ in 0..ITERS {
        let _ = std::hint::black_box(load_skin("dracula", &themes).unwrap());
    }
    let skin_ns = t1.elapsed().as_nanos() / u128::from(ITERS);
    println!("load_skin (TOML file):  {skin_ns} ns/iter  (budget: 20_000_000 ns)");

    // SC-005: both paths must stay under 20 ms each.
    assert!(
        builtin_ns < 20_000_000,
        "builtin resolve too slow: {builtin_ns} ns"
    );
    assert!(
        skin_ns < 20_000_000,
        "skin file load too slow: {skin_ns} ns"
    );
}
