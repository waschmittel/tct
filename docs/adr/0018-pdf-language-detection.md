# PDF language: detected from the text, drives quotes and alert titles

## Decision

The PDF is typeset in the language of the card, found automatically by `whichlang` (`export/lang.rs`) over the title and description without code (fenced blocks and inline code say nothing about language). There is no setting, flag or stored field. `whichlang` has no confidence value and always answers (`Pizza` → Italian), so it is only asked about text with at least `MIN_WORDS` (4) words and `MIN_LETTERS` (15) letters (words = letter runs; tokens with `://` or `@` skipped). Below that, or if the answer is not one of ours (en, de, fr, es, it, nl, pt, sv, tr), the PDF is English.

Detection runs on the card at export time, so the export dialog shows the same result in its bottom frame (`Language: Deutsch`, or `English (default)` when unsure or unsupported).

## What depends on it

- **Quotes:** straight `"` and `'` become the language's typographic quotes (en/nl/pt/tr `“…” ‘…’`, sv `”…” ’…’`, de `„…“ ‚…‘`, fr `« … »` with narrow no-break spaces, es/it `«…»`). Open/close is decided from the neighbors, looking through emphasis markers. Apostrophes between letters are always `’`; a quote after a digit stays straight (5", 6'); code spans and blocks are never touched. `...` becomes `…` in every language.
- **Alert titles:** `Lang::alert_title` (`Hinweis`, `Tipp`, `Wichtig`, `Vorsicht`, `Warnung`, …).

The read view is unaffected (ADR-0015: it never changes the typed text).

## Rejected

- An explicit per-board/card language or `--lang`: deterministic, but a persisted-format change and more UI; detection with an English fallback was the wanted scope. It can be added in front of `Lang::of` later.
- `whatlang` (the first implementation): trigram profiles confuse short, unaccented text — `Bonjour ma cherie, je t'aime beaucoup.` came out as German (confidence 0.16, 0.36 repeated twelve times, since repeating does not widen the gap), and most short sentences in all seven languages fell below its 0.9 threshold. Lowering the threshold misclassifies English (`Hello world` → Dutch), so the method had to change.
- `lingua` (restricted to our seven languages): real confidence values and the best accuracy on short text, but +33 MB in the release binary (embedded models; 11 → 44 MB). Not worth it for quotes and alert titles.
- Guessing below the confidence threshold: a wrong guess puts German quotes into an English text; English is the safe default.

## Limits

Mixed-language cards get the dominant language. `whichlang` only knows 16 languages, so Polish reads as Turkish and Danish/Norwegian as Swedish. Letters and words that cannot occur in Turkish (`ąćęłńśźż`, `jest`, `nie`, …) or Swedish (`æøþð`, `jeg`, `ikke`, `og`, …) veto that answer (`Lang::contradicted_by`), so those texts stay English; Danish or Norwegian text without any of them is still taken for Swedish. Adding Polish, Danish or Norwegian properly needs a different detector (`lingua`, `whatlang`).

A text of fewer than four words stays English even when obvious (`Bonjour ma cherie`), and the length gate is a heuristic: a short text in an unsupported language may still be read as one of ours. The size cost is about 0.3 MB.
