# The read view never moves characters; `>>>` opens a multiline quote

## Preview invariant

The details (read) view may restyle the Markdown source but must not change it so that characters move: the preview keeps the editor's layout and line breaks. Substituting a glyph for one of the same width is fine (pipes become box-drawing pipes, the delimiter row becomes a rule); adding, removing or shifting characters is not.

## Multiline quotes

A line that is just `>>>` opens a blockquote and the next such line closes it, as in GitLab-flavored Markdown (`is_quote_fence`, `src/ui/markdown.rs`). `citation_marker_len` excludes the fence, so the editor does not auto-continue it as a `>>>` citation.

- **Editor:** unchanged apart from the fence; lines inside are ordinary text.
- **Details view:** fences and inner lines are yellow, inner lines italic. No prefix or indent is added, so the text sits exactly where it was typed.
- **PDF** (`src/export/blocks.rs`): inner lines become quoted text blocks (`quote_depth + 1`), blank lines inside are kept.

## Considered and rejected

- **A `│ ` bar before each inner line in the preview.** Clearer, but it adds characters and shifts the text, violating the invariant.
- **Quoting inner lists/tables.** Inner lines are plain quoted text; nesting other block syntax is out of scope.

## Consequences

- An unclosed `>>>` quotes the rest of the description.
- Tests: `quote_fence_*` in `ui/markdown.rs`, `multiline_quote_*` in `export/blocks.rs`, `enter_on_quote_fence_*` in the editor, snapshot `card_detail_multiline_quote`; examples `00-showcase` and `08-lists-and-quotes`.
