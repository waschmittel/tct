# PDF themes: Mono keeps the grays, only color differs

`Theme::Mono` (the default PDF export, for monochrome printers) uses black text and black table rules, plus the *same grays* as `Theme::Color`: secondary text, light rules, quote text and bars, the tint behind inline code and code blocks, the page header/footer. Only chromatic things differ: label chips are outlined instead of filled, and the accent rule under the title is black. `Color` is the same layout with color. A test (`mono_keeps_the_modern_layout_so_only_colour_differs`) pins every text position across themes.

## Why

The first Mono theme was pure black on white, so nothing could dither: code blocks and inline code were drawn as thick black outlines, quotes as black. That looked heavy and harsh, and monochrome printers render light grays acceptably. Gray carries hierarchy (secondary vs. primary, code vs. prose) that outlines carried badly.

## Considered and rejected

- **Pure black Mono (the first design).** Reliable dithering-free output, but code blocks looked like form fields.
- **A third "gray" theme.** Another `--style` value and another file-name collision for no benefit; Mono simply got better.
- **Gray label chips.** A chip's fill is its identity color; a gray fill says nothing, an outline still reads as a tag.

## Consequences

- `Flow::muted()` and `rule_color()` no longer depend on the theme; the code-frame drawing (and its page-break handling) is gone.
- Examples: all cases in `docs/export-examples/` are Mono; only `00-showcase` also exists in color (`docs/export-examples/color/`).
