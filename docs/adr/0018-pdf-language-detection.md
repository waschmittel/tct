# PDF language: detected from the text, drives quotes and alert titles

## Decision

The PDF is typeset in the language of the card, found automatically by `whatlang` (`export/lang.rs`) over the title and description without code (fenced blocks and inline code say nothing about language). There is no setting, flag or stored field. If detection is not `is_reliable()`, or the language is not one of ours (en, de, fr, es, it, nl, pt), the PDF is English.

Detection runs on the card at export time, so the export dialog shows the same result in its bottom frame (`Language: Deutsch`, or `English (default)` when unsure or unsupported).

## What depends on it

- **Quotes:** straight `"` and `'` become the language's typographic quotes (en/nl/pt `“…” ‘…’`, de `„…“ ‚…‘`, fr `« … »` with narrow no-break spaces, es/it `«…»`). Open/close is decided from the neighbors, looking through emphasis markers. Apostrophes between letters are always `’`; a quote after a digit stays straight (5", 6'); code spans and blocks are never touched. `...` becomes `…` in every language.
- **Alert titles:** `Lang::alert_title` (`Hinweis`, `Tipp`, `Wichtig`, `Vorsicht`, `Warnung`, …).

The read view is unaffected (ADR-0015: it never changes the typed text).

## Rejected

- An explicit per-board/card language or `--lang`: deterministic, but a persisted-format change and more UI; detection with an English fallback was the wanted scope. It can be added in front of `Lang::of` later.
- Guessing below the confidence threshold: a wrong guess puts German quotes into an English text; English is the safe default.

## Limits

Mixed-language cards get the dominant language. Short cards fall back to English.
