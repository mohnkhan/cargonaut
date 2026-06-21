#!/usr/bin/env bash
# Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# Seed a realistic two-pane demo tree for the README GIF (docs/demo.tape).
#
# Writes under $DEMO_ROOT (default /tmp/cargonaut-demo) — tmpfs, never the SSD
# (Constitution §V). Idempotent: wipes and recreates on each run.
set -euo pipefail

DEMO_ROOT="${DEMO_ROOT:-/tmp/cargonaut-demo}"
LEFT="$DEMO_ROOT/left"
RIGHT="$DEMO_ROOT/right"

rm -rf "$DEMO_ROOT"
mkdir -p "$LEFT" "$RIGHT"

# ── LEFT pane: a believable project checkout ────────────────────────────────
mkdir -p "$LEFT/src" "$LEFT/assets" "$LEFT/.git"
cat > "$LEFT/README.md" <<'EOF'
# voyager
A small example project used in the Cargonaut demo.
EOF
cat > "$LEFT/Cargo.toml" <<'EOF'
[package]
name = "voyager"
version = "0.3.1"
edition = "2021"
EOF
printf 'fn main() {\n    println!("hello");\n}\n' > "$LEFT/src/main.rs"
printf '// helpers\npub fn add(a: i32, b: i32) -> i32 { a + b }\n' > "$LEFT/src/lib.rs"
head -c 24576 /dev/urandom > "$LEFT/assets/logo.png" 2>/dev/null || : # ~24K "image"
head -c 4096  /dev/urandom > "$LEFT/assets/icon.ico" 2>/dev/null || :
printf 'target/\n*.log\n' > "$LEFT/.gitignore"
ln -sf README.md "$LEFT/READSME.lnk" 2>/dev/null || :
: > "$LEFT/build.log"

# ── RIGHT pane: a downloads/scratch dir ─────────────────────────────────────
mkdir -p "$RIGHT/archives"
printf 'release notes ...\n' > "$RIGHT/NOTES.txt"
head -c 131072 /dev/urandom > "$RIGHT/dataset.bin" 2>/dev/null || :  # ~128K
head -c 65536  /dev/urandom > "$RIGHT/archives/backup.tar.gz" 2>/dev/null || :
printf '#!/bin/sh\necho hi\n' > "$RIGHT/run.sh"; chmod +x "$RIGHT/run.sh"
cat > "$RIGHT/config.toml" <<'EOF'
[ui]
theme = "commander-dark"
mouse = true
EOF

echo "[seed] demo tree ready: $DEMO_ROOT (left=$(ls -1 "$LEFT" | wc -l) entries, right=$(ls -1 "$RIGHT" | wc -l) entries)"
