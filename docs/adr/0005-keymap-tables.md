# Keymap tables drive dispatch and the help overlay

Each modal key surface (Board Selector, Board View, Card Detail) defines a `KEYMAP: &[Binding<Action>]` table (`src/input/keymap.rs` holds the generic `Binding`, `lookup`, `help_rows`). A binding row carries the physical key, an `Action` enum variant, and the help-overlay text (key display, description, section). Key dispatch (`lookup` → `run(action)`) and the help overlay (`ui/mod.rs::render_help`) read the same table.

## Why

A keybinding existed in four places: the input match arm, the help overlay rows, the status-bar hints, and the README table. CLAUDE.md carried a "Keep in Sync" checklist because discipline was doing the type system's job; rows drifted. With the table, a binding is defined once; help can no longer disagree with dispatch. The `help_layout_covers_all_keymap_sections` test guards the remaining manual point (section lists in `render_help`).

## Considered and rejected

- **Closures in the table instead of an `Action` enum.** Rejected — `&'static` tables can't hold capturing closures, and an enum keeps actions inspectable and testable without an `App`.
- **Driving the description editor's keys from a table.** Rejected — its keys (Ctrl-chords, Tab nesting, auto-continue) live on the `MarkdownEditor` insert handler per ADR-0003 and are mostly modifier-based; its help section stays hand-written in `render_help`.
- **Generating the README keybinding tables.** Rejected for now — README is prose-formatted and edited rarely; a generator isn't earned yet. README stays a manual sync point.

## Consequences

- New keybinding = one `Binding` row + one `Action` arm in the same file; help is generated.
- Keys requiring free-form handling (digit prefixes, chords) would bypass the table via an escape hatch in `handle()`; none exist today.
- Status-bar hints remain mode-level strings in `ui/status_bar.rs`, not per-key.
