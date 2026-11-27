# PDF fonts: DejaVu, chosen for glyph coverage

## Decision

The PDF is set in the bundled **DejaVu Sans** (regular, bold, oblique, bold oblique) and **DejaVu Sans Mono**. The deciding criterion is **glyph coverage in one font**, not looks: the exporter has no per-character font fallback (a run is measured and drawn in one face), so a character the face lacks becomes `?` (PDF/A forbids `.notdef`, ADR-0014's validation fails on it).

DejaVu covers everything the demos and the typography use: Latin extended, Greek, Cyrillic, typographic quotes (incl. U+202F for French guillemets), arrows, `✓`, `★`, box-drawing, and a good set of monochrome emoji.

## Rejected

Evaluated against the characters of the demo PDFs (glyph counts of the regular face):

- **Atkinson Hyperlegible Next/Mono** (~360 glyphs): best legibility, but no Greek, Cyrillic, arrows, `✓`, `★`, `µ`, Vietnamese, U+202F, `◦`; needs a fallback font.
- **Fira Sans/Mono**: no `✓`, `★`, U+202F; Fira Mono lacks Vietnamese `ạ`.
- **IBM Plex Sans/Mono**: no `★`; Plex Mono lacks Greek `α`; no checkboxes.
- **Noto Sans/Mono**: no arrows or `✓` in the sans face (they live in Noto Sans Symbols); Mono lacks `✓ ★`. Large (~800 KB per face).
- **Inter + JetBrains Mono**: the closest (no `☐ ☑ ─` only), but no monochrome emoji, and Inter has no mono of its own.
- **Noto Emoji** as a fallback: variable font only, and per-character fallback does not exist in the layout code.
- **Apple Color Emoji**: color bitmaps, not embeddable in PDF/A.

Nothing but DejaVu covers the emoji range in a single font, so any replacement needs per-character font fallback (split runs at font boundaries in `atoms`/`wrap`/`draw_line`) — not worth it yet.

## Consequences

- Characters DejaVu lacks (CJK, Arabic, color/ZWJ emoji) show as `?`; the showcase case `03-unicode-and-missing-glyphs` documents this.
- `export/pdf/tests/glyphs.rs` pins the decision: every character of every demo (`corner_cases()`), the fixed texts of all languages, quotes and drawn markers must have a glyph in the four proportional faces (code in the mono face, which lacks emoji); the missing-glyph case must still show missing glyphs, and they must render as `?`. A new font must pass these tests; adding demo text with an uncovered character fails them.
- Revisit if per-character fallback is built: Atkinson Hyperlegible (+ DejaVu fallback) would then be the legibility-first option.
