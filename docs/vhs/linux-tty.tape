# vhs tape: exercises the degraded rendering tier (TERM=linux) and the
# shift-free grab-mode card moves. vhs's xterm-based terminal renders more
# glyphs than a real console font, but TERM=linux makes the binary take
# the same code path a real console would: 16-color quantization,
# double-line selection border, `x` check marks. Also the only tape that
# can demonstrate card moves — vhs cannot send Shift+Arrow at all.
# Regenerate the golden via docs/vhs/check.sh, not raw vhs.

Output docs/vhs/golden/linux-tty.txt

Set Shell bash
Set FontSize 14
Set Width 1200
Set Height 700
Set TypingSpeed 40ms
Set Padding 10

Hide
# Fixed data dir (not mktemp): the path shows bottom-right in the status
# bar, so it must be deterministic for the frame-text golden.
Type `cargo build -q && cp target/debug/tct ./tct && export TCT_DATA_DIR="/tmp/tct-vhs-linux" && rm -rf "$TCT_DATA_DIR" && TCT=./tct ./docs/vhs/seed.sh >/dev/null && clear`
Enter
Sleep 3s
Show

# Start with the Linux-console TERM and open the Demo board
Type "TERM=linux ./tct"
Enter
Sleep 2s
Enter
Sleep 1.5s

# Grab the selected card (status chip flips to MOVE), move it around
# with plain arrows, then drop it back where it started
Type "m"
Sleep 800ms
Down
Sleep 800ms
Right
Sleep 800ms
Left
Sleep 800ms
Up
Sleep 800ms
Type "m"
Sleep 1s

# Card detail shows the console-safe checklist marks
Enter
Sleep 1.5s
Escape
Sleep 1.5s
