#!/bin/bash
# tmpfs-status.sh — report what's symlinked into the tmpfs build dir.
# Usage: tmpfs-status.sh <tmpfs_root>
set -euo pipefail

TMPFS_ROOT="${1:?usage: $0 <tmpfs_root>}"
LINKS=("target")

printf "[tmpfs-status] tmpfs root: %s\n\n" "$TMPFS_ROOT"

any_linked=0
for p in "${LINKS[@]}"; do
    if [ -L "$p" ]; then
        link_target="$(readlink "$p")"
        if [[ "$link_target" == "$TMPFS_ROOT"* ]]; then
            if [ -e "$link_target" ]; then
                size="$(du -sh "$link_target" 2>/dev/null | cut -f1 || echo '?')"
                printf "  [link]  %-22s → %s  (%s)\n" "$p" "$link_target" "$size"
            else
                printf "  [dangling]  %-18s → %s  (target missing — likely post-reboot; run: make tmpfs-setup)\n" "$p" "$link_target"
            fi
            any_linked=1
        else
            printf "  [warn]  %-22s → %s   (NOT in tmpfs root)\n" "$p" "$link_target"
        fi
    elif [ -d "$p" ]; then
        size="$(du -sh "$p" 2>/dev/null | cut -f1)"
        printf "  [real]  %-22s (real directory, %s — on SSD)\n" "$p" "$size"
    else
        printf "  [none]  %-22s (does not exist)\n" "$p"
    fi
done

echo
if [ -d "$TMPFS_ROOT" ]; then
    printf "Total tmpfs usage:\n"
    du -sh "$TMPFS_ROOT" 2>/dev/null | sed 's/^/  /'
    echo
    df -h "$TMPFS_ROOT" 2>/dev/null | head -2 | sed 's/^/  /'
else
    echo "  $TMPFS_ROOT does not exist (run: make tmpfs-setup)"
fi

if [ "$any_linked" -eq 0 ]; then
    echo
    echo "  No directories are currently linked. Run: make tmpfs-setup"
fi
