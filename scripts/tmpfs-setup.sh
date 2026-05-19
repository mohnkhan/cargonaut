#!/bin/bash
# tmpfs-setup.sh — relocate write-heavy build dirs to tmpfs to spare the SSD.
#
# For each pair (local_path : tmpfs_subdir) in $LINKS:
#   1. If local_path is already the right symlink → no-op (idempotent).
#   2. If local_path is a wrong symlink → remove it.
#   3. If local_path is a real dir with content → rsync into tmpfs first, then
#      remove the real dir (no data loss; existing build artifacts preserved).
#   4. Create the symlink.
#
# Then add the symlink names to .git/info/exclude (local-only) so they don't
# pollute `git status`. The shared .gitignore is untouched.
#
# Usage: tmpfs-setup.sh <tmpfs_root>
# See docs/dev-tmpfs.md for the design + caveats.
set -euo pipefail

TMPFS_ROOT="${1:?usage: $0 <tmpfs_root>}"

# (local_path : tmpfs_subdir) — the local path is what tools reference; the
# tmpfs subdir is the actual storage. Names differ where slashes would create
# nested subdirs we don't want under TMPFS_ROOT.
# Only directories that are 100% gitignored build output.
# `target/` is the entire Cargo workspace build output — the only large
# write-heavy tree in a typical Rust workspace.
LINKS=(
    "target:target"
)

migrate() {
    local local_path="$1" tmpfs_subdir="$2"
    local target_dir="$TMPFS_ROOT/$tmpfs_subdir"

    if [ -L "$local_path" ]; then
        if [ "$(readlink -f "$local_path")" = "$(readlink -f "$target_dir" 2>/dev/null || echo "$target_dir")" ]; then
            if [ ! -d "$target_dir" ]; then
                mkdir -p "$target_dir"
                echo "  [revive]  $local_path → $target_dir (recreated tmpfs target — likely post-reboot)"
            else
                echo "  [ok]      $local_path → $target_dir (already linked)"
            fi
            return 0
        fi
        echo "  [replace] $local_path was a symlink to $(readlink "$local_path"); replacing"
        rm "$local_path"
    fi

    mkdir -p "$target_dir"

    if [ -d "$local_path" ] && [ ! -L "$local_path" ]; then
        if [ -n "$(ls -A "$local_path" 2>/dev/null)" ]; then
            echo "  [migrate] $local_path → $target_dir (preserving existing content)"
            rsync -a --remove-source-files "$local_path/" "$target_dir/"
            find "$local_path" -depth -type d -empty -delete
        fi
        if [ -d "$local_path" ]; then
            rmdir "$local_path" 2>/dev/null || rm -rf "$local_path"
        fi
    fi

    mkdir -p "$(dirname "$local_path")"
    ln -s "$target_dir" "$local_path"
    echo "  [link]    $local_path → $target_dir"
}

echo "[tmpfs-setup] tmpfs root: $TMPFS_ROOT"
mkdir -p "$TMPFS_ROOT"

for entry in "${LINKS[@]}"; do
    local_path="${entry%%:*}"
    tmpfs_subdir="${entry##*:}"
    migrate "$local_path" "$tmpfs_subdir"
done

# Hide the symlinks from `git status` via the local-only exclude file. We
# intentionally do NOT modify the shared .gitignore — this is a per-checkout
# developer choice.
EXCLUDE=".git/info/exclude"
if [ -d ".git" ]; then
    mkdir -p "$(dirname "$EXCLUDE")"
    touch "$EXCLUDE"
    for entry in "${LINKS[@]}"; do
        local_path="${entry%%:*}"
        pattern="/$local_path"
        if ! grep -Fxq "$pattern" "$EXCLUDE" 2>/dev/null; then
            echo "$pattern" >> "$EXCLUDE"
            echo "  [exclude] added '$pattern' to $EXCLUDE"
        fi
    done
fi

echo
echo "[tmpfs-setup] done."
echo "  Verify with:  make tmpfs-status"
echo "  Revert with:  make tmpfs-teardown    (use WIPE=1 to also rm -rf the tmpfs dir)"
