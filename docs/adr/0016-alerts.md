# Alerts: tagged quotes, color + capitalized kind, `Ctrl+G` chord

## Syntax

A quote whose first line is `[!note]`, `[!tip]`, `[!important]`, `[!caution]` or `[!warning]` (any case) is an alert, as on GitHub/GitLab:

- `> [!note]` and the `>` lines that follow (the tag counts only on the first line of a single-`>` quote);
- `>>> [!note]` … `>>>` for multiline bodies (ADR-0015). The opener with a tag is a fence like bare `>>>`; only a bare `>>>` closes.

Unknown tags are ordinary quotes. Kinds and colors as on GitHub: note blue, tip green, important purple, warning gold, caution red. Octicons are PDF-only.

## Rendering

- **Details view:** the preview invariant (ADR-0015) holds — nothing after the tag moves. The tag, which is alone on its line, becomes the kind in bold capitals (`NOTE`) padded to the tag's display width; the whole alert is in the alert color. No icons in the terminal: emoji have ambiguous widths, ignore the foreground color and are missing on the Linux console. The PDF keeps GitHub's icons.
- **PDF:** `Block::Text` carries `InAlert { kind, title }`; a bar in the alert color runs along all lines (blank lines inside become empty text lines so the bar is continuous), the title line is GitHub's Primer Octicon (`info`, `light-bulb`, `report`, `alert` for warning, `stop` for caution; exact 16px SVG paths in `assets/octicons/`, MIT, parsed by `kurbo` and filled as vector paths) plus the kind in bold capitals, in that color. Mono theme: black bar and title; body text is normal ink, not italic.

## Editing

`Ctrl+G` inside a quote (inside a code block it does not count) arms a one-shot chord like `Ctrl+T` in tables: `n t i c w` set or change the kind, `x` returns to a plain quote. `>` quotes get/lose a `> [!kind]` title line; `>>>` quotes get/lose the tag on the fence. One undo step. Outside a quote `Ctrl+G` does nothing; `Ctrl+A` stays line start (the reason it was not used).

## Considered and rejected

- **Custom titles after the tag** (`>>> [!note] Title`): out of scope; such a line is an ordinary quote line.
- **Icons/emoji in the terminal:** emoji have ambiguous widths, ignore the foreground color and are missing on the Linux console; the terminal shows only the kind in capitals.
- **Hand-drawn icons in the PDF:** replaced by the exact Octicons.

## Consequences

Tests: `alert.rs`, `alert_edit.rs`, `citation_alert_*`/`fenced_alert_*` in `export/blocks.rs`, `alerts_*` in `ui/markdown.rs` and `export/pdf.rs`, `ctrl_g_*` in the editor, snapshot `card_detail_alerts`; example `15-alerts` (+ showcase).
