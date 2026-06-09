# Dialog Kind and Insert Target as traits, not flat enums

`DialogKind` (11 variants) and `InsertTarget` (13 variants) are replaced by `trait Dialog` and `trait InsertHandler`. Each variant becomes a struct owning its own state. App holds `Option<Box<dyn Dialog>>` and `Option<Box<dyn InsertHandler>>`.

Both traits return an outcome enum (`DialogOutcome` / `InsertOutcome`) whose `Confirm`/`Apply` variant carries a `Command` (per ADR-0002). The App's event loop applies the Command via Board Editor.

## Why

Each enum variant had its render in `ui/`, its input handling in `input/`, and its payload stashed on `App`. Adding a Dialog Kind or Insert Target meant edits in three files (CLAUDE.md already acknowledged this for input handlers). Trait-based handlers concentrate render + input + state in one struct, one file.

## Considered and rejected

- **Keep flat enums, just split files harder.** Rejected — the friction is in the cross-file coupling, not the file size.
- **Use generic associated types instead of an outcome enum.** Rejected — `Box<dyn Dialog>` doesn't accept GATs; outcome enums are simpler and let multiple dialogs return the same `Command`.

## Consequences

- New Dialog Kind = new file in `src/dialog/`.
- New Insert Target = new struct in `src/insert/` (grouped by widget kind: `line_editor`, `markdown_editor`, `date_picker`, `picker`).
- Insert Handlers share `LineInput` / `TextAreaInput` base structs to avoid buffer/cursor boilerplate.
- Confirmation dialogs store raw IDs and build a `Command` at `Enter`, not the `Command` itself, so they can refresh from the current Board Editor state at confirmation time.
