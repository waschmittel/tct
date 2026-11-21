# In-editor Find and Find & Replace: docked bar, regex default, Tab-focusable chips

The description editor provides Find (`Ctrl+F`) and Find & Replace (`Ctrl+R`, plus `F3`/`Shift+F3` to repeat) as an internal sub-state (`FindState`) on `MarkdownEditor` rendered as a docked bar at the bottom of the card detail popup. Regex search is enabled by default (case-insensitive), with `[Aa]` (case-sensitive) and `[.*]` (regex) toggles plus `[Replace]` and `[All]` actions exposed as Tab-focusable chips alongside direct shortcuts. Match highlighting uses `tui-textarea-2`'s built-in `search` feature without patching the vendored crate.

## Why

1. **`Ctrl+R` instead of `Ctrl+H`.** `Ctrl+H` is already bound to contextual Help in `MarkdownEditor` (ADR-0011). `Ctrl+R` is unused (redo lives on `Ctrl+Y` / `Ctrl+Shift+Z`) and familiar from terminal editors.
2. **Docked bar inside `MarkdownEditor` instead of a floating `Dialog` or new `AppMode`.** Keeping `find: Option<FindState>` on `MarkdownEditor` keeps the text live and scrollable during incremental search, leaves `AppMode` and `enforce_mode_invariants()` untouched, lets `Esc` dismiss the bar without triggering `ConfirmCancelEdit`, routes bracketed paste into the active input, and keeps `F1`/`Ctrl+H` working over the bar.
3. **Regex enabled by default.** Card descriptions are plain text and Markdown; power users frequently search with patterns and capture-group replacements (`$1`), while literal matching is one toggle away (`[.*]`, `Alt+X`, or `F5`). When pre-filling Find from a single-line selection in regex mode, the selection is escaped with `regex::escape` so selecting `[x]` or `(foo)` matches itself immediately.
4. **Tab-focusable chips.** `TERM=linux` and default macOS terminals do not reliably transmit `Alt+<letter>`, `Ctrl+Enter`, or `Shift+Enter`. Making `[Replace]`, `[All]`, `[Aa]`, and `[.*]` focusable in the `Tab`/`Shift+Tab` cycle (activated with `Enter` or `Space`) guarantees every option and action works on every terminal, while `Alt+C`/`F4`, `Alt+X`/`F5`, and `Alt+A`/`Ctrl+G` remain fast shortcuts.
5. **No vendor patch.** `vendor/tui-textarea-2` already ships `features = ["search"]` (backed by `regex = "1"`, already a direct dependency) with soft-wrap and hanging-indent clipping. Using a named ANSI highlight style (`bg(Color::Blue).fg(Color::White)`) stays readable on `TERM=linux` without expanding `vendor/patches/tui-textarea-2.patch`.
6. **Reverse-order, undo-grouped Replace All.** Collecting all match spans first and applying them bottom-to-top, right-to-left inside one `table_edit::grouped` block prevents coordinate shift and self-matching infinite loops (e.g. `"a" -> "aa"` or zero-length `^`), and lets one `Ctrl+Z` revert the whole bulk edit (including table reflow and list renumbering when paste mode is off).

## Considered and rejected

- **Floating modal `Dialog`.** Obscures the text being searched and complicates returning to the live editor without losing incremental scroll.
- **Patching `vendor/tui-textarea-2` for a separate active-match highlight.** Keeps the vendored fork smaller; the textarea cursor cell (priority 30 > search priority 20) already marks the active match.
- **Literal-only search or literal default.** Rejected in favor of regex-by-default with a one-key toggle to literal mode.

## Consequences

- `Cargo.toml` enables `features = ["search"]` on `tui-textarea-2`; zero new crates are added.
- `MarkdownEditor` owns `find: Option<FindState>` and `find_history: FindHistory` (`src/insert/markdown_editor/find.rs`).
- Single replace and Replace All each form one undo step and reformat tables / renumber lists unless paste mode is active.
- `Esc` closes the bar, clears highlights, and leaves the cursor on the current match; `F3`/`Shift+F3` repeat the last search with the remembered options.
