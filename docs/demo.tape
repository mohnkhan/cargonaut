# Cargonaut demo tape (vhs).
#
# Regenerate the GIF with:  make demo-gif
# (needs vhs + ttyd + ffmpeg; see docs/RELEASING.md). The Makefile target seeds
# a demo tree under /tmp (tmpfs, Constitution §V) and puts the release binary on
# PATH before invoking vhs. Keys are limited to those vhs supports (no bare
# F-keys), so this demos keyboard + scroll-wheel navigation.

Output docs/demo.gif

Set Shell bash
Set FontSize 15
Set Width 1180
Set Height 640
Set Padding 10
Set TypingSpeed 100ms

# Launch the real binary against the seeded demo tree (hidden from the capture).
Hide
Type "cargonaut /tmp/cargonaut-demo/left /tmp/cargonaut-demo/right --theme commander-dark"
Enter
Sleep 2s
Show

# Settle on the dual-pane view.
Sleep 1200ms

# Walk down to the src/ directory (last entry) and descend into it, then back up.
Down@450ms 5
Sleep 700ms
Enter
Sleep 1400ms
Backspace
Sleep 900ms

# Switch to the right pane, browse, and use the scroll wheel.
Tab
Sleep 700ms
Down@500ms 3
Sleep 500ms
ScrollDown 2
Sleep 800ms
ScrollUp 1
Sleep 700ms

# Back to the left pane; open the quick-cd prompt (Alt-c) to show inline path entry, cancel.
Tab
Sleep 600ms
Alt+c
Sleep 1600ms
Escape
Sleep 1300ms
