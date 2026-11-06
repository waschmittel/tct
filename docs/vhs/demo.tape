# vhs tape: drives the real tct binary in a fresh TCT_DATA_DIR.
# Produces a demo gif and a final-frame text golden (docs/vhs/golden/demo.txt).
# vhs writes every sampled frame to the .txt; check.sh trims it to the final
# frame (intermediate frames race against status-message timing and differ
# between runs). Regenerate the golden via docs/vhs/check.sh, not raw vhs.
# Regression check: docs/vhs/check.sh
#
# Note: vhs cannot send Shift+Arrow; card moves are demonstrated via
# grab mode (m + arrows) in linux-tty.tape.

Output docs/vhs/demo.gif
Output docs/vhs/golden/demo.txt

Set Shell bash
Set FontSize 14
Set Width 1200
Set Height 700
Set TypingSpeed 40ms
Set Padding 10

Hide
# Fixed data dir (not mktemp): the path shows bottom-right in the status
# bar, so it must be deterministic for the frame-text golden.
Type `cargo build -q && cp target/debug/tct ./tct && export TCT_DATA_DIR="/tmp/tct-vhs-demo" && rm -rf "$TCT_DATA_DIR" && TCT=./tct ./docs/vhs/seed.sh >/dev/null && clear`
Enter
Sleep 3s
Show

# Board selector: open the Demo board
Type "./tct"
Enter
Sleep 2s
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

# Long lists scroll: navigate down so the viewport scrolls and card is focused
Down
Sleep 400ms
Down
Sleep 400ms
Down
Sleep 400ms
Down
Sleep 400ms
Down
Sleep 400ms
Down
Sleep 800ms

# Switch boards (k/j or ./,): selection and list scroll offset are remembered
Type "k"
Sleep 1.5s
Down
Sleep 600ms
Type "j"
Sleep 1.5s
Up
Sleep 600ms
Type "g"
Sleep 800ms

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

# Edit description in markdown editor
Type "e"
Sleep 800ms

# Contextual help overlay: inspect editor shortcuts
Ctrl+H
Sleep 2s
Escape
Sleep 800ms

# Citations with auto-continuation
Type " "
Enter
Type "> Root cause analysis:"
Sleep 600ms
Enter
Sleep 500ms
Type "Auth token expires after 15 min."
Sleep 800ms
Enter
Sleep 400ms
Enter
Sleep 400ms

# Numbered list with auto-numbering, indentation (Tab), and un-indentation (Shift+Tab)
Type "1. Check token expiration"
Sleep 600ms
Enter
Sleep 600ms
Type "Inspect refresh handler"
Sleep 600ms
Tab
Sleep 800ms
Enter
Sleep 600ms
Type "Add regression test"
Sleep 600ms
Shift+Tab
Sleep 800ms

# Markdown formatting shortcuts: list bullet, bold, and inline code
Enter
Sleep 400ms
Enter
Sleep 400ms
Ctrl+L
Sleep 500ms
Ctrl+B
Sleep 400ms
Type "Fix"
Sleep 400ms
Right 2
Type " in "
Sleep 300ms
Ctrl+K
Sleep 400ms
Type "auth.rs"
Sleep 400ms
Right
Sleep 800ms

# Tables: Ctrl+T inserts one; typing reflows it, Tab walks the cells (and appends
# a row from the last cell), Ctrl+T then c/a adds a column and cycles alignment,
# Enter below the last row leaves the table
Enter
Enter
Ctrl+T
Sleep 600ms
Type "Check"
Tab
Type "Result"
Sleep 400ms
Ctrl+T
Sleep 800ms
Type "c"
Sleep 400ms
Type "Notes"
Sleep 400ms
Ctrl+T
Type "a"
Ctrl+T
Type "a"
Sleep 800ms
Tab
Type "Refresh"
Tab
Type "ok"
Tab
Type "ttl fixed"
Sleep 400ms
Tab
Sleep 400ms
Type "Expiry"
Tab
Type "15 min"
Tab
Type "n/a"
Sleep 800ms

# Emphasis in a table cell without a selection formats the whole cell and reflows:
# Ctrl+B bold, Ctrl+E adds italic (bold italic), Ctrl+B again drops the bold
Ctrl+B
Sleep 800ms
Ctrl+E
Sleep 800ms
Ctrl+B
Sleep 800ms
Enter
Sleep 500ms
Type "Table rows stay aligned."
Sleep 600ms

# Outside tables the keys open a pair; pressed inside formatted text they remove it
Enter
Sleep 300ms
Ctrl+B
Type "bold"
Right 2
Type " and "
Ctrl+E
Type "italic"
Right
Sleep 600ms
Left 3
Ctrl+E
Sleep 1s

# Save without closing (Ctrl+S), then save and close (Ctrl+Q)
Ctrl+S
Sleep 1.2s
Ctrl+Q
Sleep 1.5s

# Date stamps in the detail frame (also toggled from the board view later)
Type "c"
Sleep 600ms
Type "d"
Sleep 1.5s
Type "c"
Sleep 400ms
Type "d"
Sleep 600ms

# Assign a label from the picker
Type "l"
Sleep 800ms
Down
Sleep 500ms
Space
Sleep 800ms
Escape
Sleep 800ms

# Set a due date in the date picker
Type "u"
Sleep 800ms
Type "2099-12-31"
Sleep 600ms
Enter
Sleep 1s

# Clear it again; the confirmation dialog guards the removal
Type "U"
Sleep 1s
Type "y"
Sleep 1s

# History: two panes, summary left and field diff right; scroll diff and inspect entries
Type "h"
Sleep 1.5s
PageDown
Sleep 1s
PageUp
Sleep 800ms
Down
Sleep 900ms
Down
Sleep 900ms
Down
Sleep 900ms
Type "x"
Sleep 900ms
Type "y"
Sleep 1.2s
Escape
Sleep 800ms

# Back to the board
Escape
Sleep 1s

# Grab the card with m, move it with the arrows, drop it
Type "m"
Sleep 600ms
Down
Sleep 600ms
Type "m"
Sleep 800ms

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
Sleep 800ms
Type "y"
Sleep 1.2s

# Show the created and updated dates in the card frames, then hide them again
Type "c"
Sleep 800ms
Type "d"
Sleep 1.5s
Type "c"
Sleep 500ms
Type "d"
Sleep 800ms

# Help overlay
Type "?"
Sleep 2s
Escape
Sleep 1.5s
