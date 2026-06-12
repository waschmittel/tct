# vhs tape: drives the real tct binary in a fresh TCT_DATA_DIR.
# Produces a demo gif and a final-frame text golden (docs/vhs/golden/demo.txt).
# Run from the repo root (builds + seeds itself):  vhs docs/vhs/demo.tape
# Regression check: docs/vhs/check.sh
#
# Note: vhs cannot send Shift+Arrow, so card-move bindings aren't shown.

Output docs/vhs/demo.gif
Output docs/vhs/golden/demo.txt

Set Shell bash
Set FontSize 14
Set Width 1200
Set Height 700
Set TypingSpeed 40ms
Set Padding 10

Hide
Type `cargo build -q && cp target/debug/tct ./tct && export TCT_DATA_DIR="$(mktemp -d)" && TCT=./tct ./docs/vhs/seed.sh >/dev/null && clear`
Enter
Sleep 3s
Show

# Board selector
Type "./tct"
Enter
Sleep 2s

# Open the Demo board
Enter
Sleep 1.5s

# Navigate cards and lists
Down
Sleep 500ms
Up
Sleep 500ms
Right
Sleep 500ms
Left
Sleep 700ms

# Create a card
Type "n"
Sleep 600ms
Type "Write release docs"
Sleep 400ms
Enter
Sleep 1.2s

# Open card detail of the first card
Type "g"
Sleep 400ms
Enter
Sleep 1.5s

# Checklist: toggle the second item, add a new one
Down
Sleep 400ms
Space
Sleep 700ms
Type "a"
Sleep 500ms
Type "Ship it"
Sleep 300ms
Enter
Sleep 1s

# Edit the description in the markdown editor
Type "e"
Sleep 800ms
Type "WIP: "
Sleep 500ms
Ctrl+S
Sleep 1s

# Back to the board
Escape
Sleep 1s

# Reorder lists
Type ">"
Sleep 800ms
Type "<"
Sleep 800ms

# Search, jump to the match, clear filters
Type "/"
Sleep 400ms
Type "dash"
Sleep 500ms
Enter
Sleep 1.2s
Type "F"
Sleep 800ms

# Archive the selected card, then restore it from the archive dialog
Type "a"
Sleep 800ms
Type "y"
Sleep 1s
Type "v"
Sleep 1s
Enter
Sleep 1.2s

# Help overlay
Type "?"
Sleep 2s
Escape
Sleep 1.5s
