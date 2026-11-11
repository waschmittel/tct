# vhs tape: screenshots for the website (run via site/showcase/build.sh).
# Every group relaunches tct so state (selection, toasts) is deterministic.
Output "/tmp/tct-site-shots.gif"
Set Shell bash
Set Theme "Catppuccin Mocha"
Set FontSize 15
Set Width 1240
Set Height 960
Set Padding 14
Set TypingSpeed 60ms

Hide
Type "cd /tmp/tct-site-home/Projects && clear" Enter
Show

# Board selector with per-board accent colors
Type "tct" Enter
Sleep 1.5s
Down
Type "c"
Down
Type "c"
Up
Up
Sleep 1s
Screenshot "site/img/shot-selector.png"
Sleep 500ms
Type "q"
Sleep 500ms

# Board view, card detail (top, matrix, footnotes)
Type "clear" Enter
Type "tct --board release" Enter
Sleep 1.5s
Right
Sleep 1s
Screenshot "site/img/shot-board.png"
Sleep 500ms
Enter
Sleep 1s
Screenshot "site/img/shot-detail.png"
Sleep 500ms
PageDown 17
Sleep 1s
Screenshot "site/img/shot-detail-scrolled.png"
Sleep 500ms
PageDown 60
Sleep 1s
Screenshot "site/img/shot-detail-end.png"
Sleep 500ms
Type "q"
Sleep 500ms

# Search hides non-matching cards
Type "clear" Enter
Type "tct --board release" Enter
Sleep 1.5s
Type "/login"
Enter
Sleep 1s
Screenshot "site/img/shot-search.png"
Sleep 500ms
Type "q"
Sleep 500ms

# Label filter
Type "clear" Enter
Type "tct --board release" Enter
Sleep 1.5s
Type "f"
Sleep 1s
Screenshot "site/img/shot-filter.png"
Sleep 500ms
Escape
Type "q"
Sleep 500ms

# Help overlay
Type "clear" Enter
Type "tct --board release" Enter
Sleep 1.5s
Type "?"
Sleep 1.5s
Screenshot "site/img/shot-help.png"
Sleep 500ms
Escape
Type "q"
Sleep 500ms

# PDF export dialog
Type "clear" Enter
Type "tct --board release" Enter
Sleep 1.5s
Right
Type "P"
Sleep 1.5s
Screenshot "site/img/shot-pdf-dialog.png"
Sleep 500ms
Escape
Type "q"
Sleep 500ms

# Markdown editor
Type "clear" Enter
Type "tct --board release" Enter
Sleep 1.5s
Right
Type "e"
Sleep 1.5s
Screenshot "site/img/shot-editor.png"
Sleep 500ms
Ctrl+q
Type "q"
Sleep 500ms
