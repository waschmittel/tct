# Footnotes: GitHub syntax, same-page placement in the PDF, `Ctrl+N`

## Syntax

As on GitHub/GitLab: a reference `text[^label]` and a definition line `[^label]: note`, anywhere in the text. Labels are any run without whitespace, `[`, `]`, `^`. A definition continues over following non-blank lines that start no other block (lazy continuation) and over paragraphs indented by four spaces after a blank line. Fenced code and inline code are never scanned. The first definition of a label wins; unreferenced definitions are not printed; references without definition stay literal.

## Rendering

- **Details view:** ADR-0015 holds — the text is unchanged and stays where it was typed (definitions are *not* moved to the bottom); references and definition labels are colored.
- **PDF:** definitions are removed from the flow; a named note is marked by its name; numbered notes (all-digit labels) are marked by a running count in order of *first reference* (GitHub's rule), whatever their digits. Names and numbers cannot collide (labels of digits are never names). References in the text are raised, smaller runs in brackets (`Style.sup`, e.g. `[1]`, `[my-note]`); the marker in front of a note is bracketed the same way. Notes are set small below a short rule at the bottom of the page; each starts with its marker as a raised, black superscript without a dot, as large as a reference in the text. The text of all notes on a page starts at one indent behind that page's longest marker (so it can differ between pages; wrapping assumes the document's longest marker so lines never outgrow a page). Notes are set apart by a little extra space, and the rule above them is a visible gray.

### Same-page invariant

A note is printed on every page that holds a reference to it. Layout therefore plans notes together with text: each page has a footnote area whose height is reserved (`Flow::bottom()`); before a line (or table row) holding references is drawn, `fits` checks that the line plus the notes not yet on the page fit above that area, `need_notes` breaks the page until they do, and `register` adds the notes. Orphan/widow groups (`lines_to_keep`) are checked as a unit, `keep_together` measurements include their notes, and repeated table headers register their references on each page.

Consequence worth knowing: a reference line may move to the next page together with its paragraph's keep-together group when the note does not fit.

### Limits

A page's area is capped (`NOTE_BUDGET`, half the content height) so body text always keeps room. A single note (or set of notes of one line) beyond the budget splits: the rest continues at the top of the next page's area (`carry`, no number repeated), cascading as needed, and trailing carry gets pages of its own. The same-page guarantee holds for every page whose notes fit the budget; beyond that nothing is lost, but it cannot hold physically.

## Editing

`Ctrl+N`: outside a definition, insert `[^n]` at the cursor and renumber the numbered footnotes (references and definitions, not names or code) to ascend by first reference, so the new one may shift later ones; its definition is placed among the numbered definitions by number — right behind the one with the highest smaller number, else in front of the one with the lowest larger number (named definitions do not take part; with no numbered definition, or for a named footnote, it goes to the very bottom, absorbing trailing blank lines). Only *pairs* (a label with a reference and a definition) are renumbered; dangling references and unreferenced definitions keep their label and the new numbers skip them, so a reference never starts resolving to another footnote's definition. A cursor inside a `[^x]` inserts behind it. The new definition is an empty `[^n]: ` (a blank line before it only at the bottom, none between consecutive definitions), cursor behind it, one undo step. With text highlighted the footnote is *named* after it instead (`label_from`: whitespace runs → `-`, brackets/carets dropped, at most 32 chars; nothing usable or all digits → numbered as without selection): the text stays, `[^name]` goes behind the selection (named footnotes never renumber the numbered ones), and if that name is already defined only the jump happens. Inside a definition (any of its lines), jump behind its first reference — one outside definitions preferred — or show "No reference" if there is none. Works in paste mode. `Ctrl+N` was Down in the textarea's emacs keys; arrows remain.

## Considered and rejected

- **Numbering by label or definition order:** differs from GitHub and makes the PDF depend on where notes are written.
- **Footnote area as plain end-of-document list:** the requirement is notes on the page of their reference.
- **A `sup` marker as Unicode superscript digits:** only 0–9, wrong metrics; a style flag draws any number at a scaled size.
- **Notes inside tables/references inside definitions:** references inside table cells are supported; references inside definitions stay literal.

## Consequences

Tests: `footnote.rs`; `ctrl_n_*` in the editor; `footnotes_*` in `ui/markdown.rs`; in `export/pdf.rs` the boundary sweep, `assert_notes_consistent` over blocks/tables/long notes/spill, determinism across themes; snapshots `card_detail_footnotes`, `description_editor_footnote`; example `16-footnotes` (+ showcase).
