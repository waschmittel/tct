# vhs tape: drives the real tct binary in a fresh TCT_DATA_DIR.
# Produces a demo gif and a final-frame text golden (docs/vhs/golden/demo.txt).
# Run from the repo root after `cargo build`:  vhs docs/vhs/demo.tape
# Regression check: docs/vhs/check.sh

Output docs/vhs/demo.gif
Output docs/vhs/golden/demo.txt

Set Shell bash
Set FontSize 14
Set Width 1200
Set Height 700
Set TypingSpeed 40ms
Set Padding 10

Hide
Type `export TCT_DATA_DIR="$(mktemp -d)" && ./docs/vhs/seed.sh >/dev/null && clear`
Enter
Sleep 1s
Show

Type "./target/debug/tct --board Demo"
Enter
Sleep 1.5s

# Navigate cards
Down
Sleep 600ms
Up
Sleep 600ms

# Open card detail
Enter
Sleep 1.5s

# Toggle second checklist item
Down
Sleep 400ms
Space
Sleep 1s

# Back to board view — final frame (the text golden) is the board view
Escape
Sleep 1.5s
