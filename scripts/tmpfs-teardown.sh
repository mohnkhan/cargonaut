#!/bin/bash
# tmpfs-teardown.sh — undo tmpfs-setup.sh.
#
# Removes the symlinks (build artifacts in tmpfs are kept by default so a
# subsequent re-setup is fast). Pass WIPE=1 as the second arg to also
# rm -rf the tmpfs root.
#
# Usage: tmpfs-teardown.sh <tmpfs_root> [<WIPE>]
set -euo pipefail

TMPFS_ROOT="${1:?usage: $0 <tmpfs_root> [WIPE]}"
WIPE="${2:-}"
LINKS=("target")

echo "[tmpfs-teardown] tmpfs root: $TMPFS_ROOT"

for p in "${LINKS[@]}"; do
    if [ -L "$p" ]; then
        link_target="$(readlink "$p")"
        rm "$p"
        echo "  [unlink]  $p (was → $link_target)"
        # Recreate an empty directory so subsequent builds don't trip on ENOENT.
        # Pass-through behavior matches a fresh checkout.
        mkdir -p "$p"
    fi
done

# Remove our entries from .git/info/exclude
EXCLUDE=".git/info/exclude"
if [ -f "$EXCLUDE" ]; then
    for p in "${LINKS[@]}"; do
        # Match exactly "/${p}" on a line of its own (the form tmpfs-setup wrote)
        sed -i "\|^/$p$|d" "$EXCLUDE"
    done
fi

if [ "$WIPE" = "1" ]; then
    if [ -d "$TMPFS_ROOT" ]; then
        echo "  [wipe]    rm -rf $TMPFS_ROOT (WIPE=1)"
        rm -rf "$TMPFS_ROOT"
    fi
else
    echo
    if [ -d "$TMPFS_ROOT" ]; then
        kept_size="$(du -sh "$TMPFS_ROOT" 2>/dev/null | cut -f1)"
        echo "  Note: $TMPFS_ROOT preserved ($kept_size of build artifacts kept)."
        echo "  To remove it: make tmpfs-teardown WIPE=1"
    fi
fi
