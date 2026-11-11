# A table with an empty top-left cell is a matrix

A pipe table whose top-left cell is empty (`Table::is_matrix`, `src/insert/table.rs`) is a *matrix*: it carries labels on the left as well as on top. Readers render it accordingly; the editor and the stored Markdown do not change.

- **Details (read) view:** the header row has no border left of the open corner, the rule under it starts with `┌` (not `├`), and the first column is bold like the header.
- **PDF:** the first column is bold; the corner has no rule above and no line to its left; heavy lines always separate labels from contents (under the header, right of the first column); column lines appear only with more than 3 *content* columns (more than 4 in all); thin row separators appear with more than 7 rows, whatever the column count, and then the outer rules are as thin as the separators; a matrix *without* row separators has no top or bottom border.
- **Normal tables** keep their own thresholds: thin column lines with more than 7 rows or more than 3 columns, no row separators.

- **Transposing (editor):** `Ctrl+T`, `t` (`Table::transpose`) swaps rows and columns of a matrix, one undo step, cursor stays on the same cell. The corner stays top-left, so the result is a matrix again; alignment is per old column and cannot carry over, so every new column gets the first data column's alignment. A table that is not a matrix refuses (toast): without row labels, rows and columns are not interchangeable.

## Why

The distinction is semantic, so it has to come from the content. Markdown pipe tables have no syntax for row headers; an empty corner cell is the convention for "this is a grid, not a list of records" and costs the author nothing. Deriving it from the data keeps the file format plain Markdown (no extension syntax, no `.version` bump, ADR-0009) and keeps the preview, the PDF and any other Markdown renderer showing the same table.

## Considered and rejected

- **A syntax extension (`|^ ...`, a front-matter flag).** Not portable; other renderers would show noise.
- **Always bolding the first column.** Wrong for record tables, where the first column is just data.
- **Thresholds counted on all columns.** A matrix's label column is not content; counting it made a 3×3 matrix look crowded enough for a grid.
- **Making the editor aware of matrices beyond transposing.** Nothing else to edit differently; the corner is just an empty cell, and a freshly created table (empty header cells) is therefore a matrix until it is filled.

## Consequences

- Typing text into the corner turns a matrix back into a normal table, in the preview and in the next export.
- Both Markdown dialects (`ui/markdown/` and `export/blocks.rs`) get the answer from the same `Table::is_matrix`, so they cannot drift.
- Tests: `matrix_*` in `src/export/pdf/`, `src/ui/markdown/`, `src/insert/table.rs`; examples `00-showcase` and `14-matrix` in `docs/export-examples/`.
