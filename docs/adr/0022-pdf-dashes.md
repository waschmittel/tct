# PDF dashes: `--`, `---` and a spaced hyphen become dashes, in the PDF only

## Decision

`typeset` (`export/blocks.rs`, the same place as quotes and `...`) turns, in non-code text of the **PDF only**:

- `--` into an en dash `–` and `---` into an em dash `—`, in every language (`a--b`, `a -- b`).
- a *spaced* hyphen ` - ` (whitespace on both sides, words on both sides) into a spaced en dash ` – `, in every language (British and continental style; it keeps the author's spacing, so it is never wrong for a spaced hyphen).

The read view, the editor and the stored text stay as typed (ADR-0015). Dashes do not depend on the detected language (ADR-0018).

## What is left alone

An unspaced hyphen (`well-known`) cannot be told from a dash, and digit ranges (`10-20`) cannot be told from dates, phone numbers, IDs or scores, so those are never touched. Also literal: `5 - 3` (two digits around a spaced hyphen), flags (`--verbose`, a `--` opening a word), arrows and table rules (`-->`, `<--`, `|--:|`), runs of four or more hyphens, `--` at the very start or end of the text, any whitespace-delimited token holding `/` or `@` (URLs, paths, addresses), list markers and horizontal rules (consumed by the block parser earlier), and code spans/blocks (never reach `typeset`).

## Rejected

- Converting digit ranges to en dashes: wrong for ISO dates, IDs, scores and negative numbers.
- Converting any unspaced hyphen: indistinguishable from compound words.
- Doing it in the read view: it would change what the user sees as typed (ADR-0015).
- A per-language style (closed em dash `one—two` in English/Spanish per Chicago, spaced en dash elsewhere): it rewrote the author's spacing, looked wrong for a spaced input, and needed a convention per language. Pandoc's `smart` likewise never touches spacing.

## Limits

A spaced hyphen as a minus between words (`a - b`) is read as a dash; between two digits it stays. A standalone `--` meaning "end of options" in prose becomes an en dash. Both spaces of ` - ` must lie in the same run; `**a** - b` and `a - **b**` work.
