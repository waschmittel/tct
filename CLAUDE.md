# CLAUDE.md — Agent Instructions for tct

## Build & Test

```sh
cargo build          # Must compile with zero errors and zero warnings
cargo test -- --test-threads=1   # Tests use shared filesystem state
```

## Architecture

- **Modal input**: `AppMode` enum in `app.rs` drives which handler in `input/` runs. Modes: BoardSelector, Normal, CardDetail, Insert, Command, Dialog, Help. `Insert` and `Dialog` are parameterless — the active handler is `Box<dyn>` on `App.insert` / `App.dialog`.
- **Dialog trait** (`src/dialog/`): One struct per dialog kind implementing `Dialog { render, handle_key, background }`. Each holds its own payload + cursor/scroll state. Returns `DialogOutcome { apply: Option<Command>, side_effect, status, follow }`. The dispatcher in `input/dialog_input.rs` interprets the outcome. See `docs/adr/0003-dialog-and-insert-as-traits.md`.
- **InsertHandler trait** (`src/insert/`): One struct per insert target. Handlers are grouped by widget kind: `line_editor.rs` (11 line inputs sharing `LineInput`), `markdown_editor.rs` (description editor over `TextAreaInput`), `date_picker.rs` (due-date picker). Returns `InsertOutcome::{Stay, Cancel, Confirm(Command), ConfirmAndOpenDialog, OpenDialog, CancelWithStatus, ConfirmSideEffect}`. Dispatcher in `input/insert.rs`.
- **Storage**: JSON files under `~/.tct/boards/`. All writes use `atomic_write` (write `.tmp`, then rename). Override path with `TCT_DATA_DIR` env var.
- **Description editing**: Lives on the `MarkdownEditor` insert handler (`src/insert/markdown_editor.rs`). Wraps `ratatui-textarea::TextArea` via `TextAreaInput` shared base. List autocontinue + renumbering + nest/unnest also live there. Renderer in `card_detail.rs::render_description_editor()` reads from the handler.
- **Markdown rendering**: `MarkdownRenderer` in `ui/markdown.rs` (refactored separately). Word-wrap at `WRAP_WIDTH` (80 chars). The description editor's cursor visual mapping uses the legacy `build_visual_map` / `source_to_visual` helpers in the same module.
- **Label colors**: `LabelColor` enum with named pastel variants + `Custom { r, g, b }`. New labels get auto-generated pastel colors via `LabelColor::generate_pastel()` which picks maximally distant hue from existing labels.
- **Board accent color**: Each board has an `accent_color: LabelColor` field in `BoardMeta`. All UI highlight/accent colors use `app.accent_color()` instead of hardcoded `Color::Cyan`. New boards auto-get a differentiated pastel color. Users cycle with 'c' in board selector. Help overlay keeps structural Cyan.
- **Search**: When active, non-matching cards are hidden (not just dimmed). Navigation skips hidden cards. First match auto-selected on search confirm.
- **Periodic reload**: `App::on_tick()` reloads board from filesystem every 15s (configurable via `reload_interval`). Skipped during editing/dialog/grab modes. Preserves selection state.

## How to Add Things

### New keybinding in board selector
1. Add match arm in `input/board_selector_input.rs`
2. Update status bar hints in `ui/status_bar.rs`
3. Update help text in `ui/mod.rs::render_help()`
4. Update README.md keybindings table

### New keybinding in board view
1. Add match arm in `input/normal.rs`
2. Update status bar hints in `ui/status_bar.rs`
3. Update help text in `ui/mod.rs::render_help()`
4. Update README.md keybindings table

### New keybinding in card detail
1. Add match arm in `input/card_detail_input.rs`
2. Update bottom hints in `ui/card_detail.rs` (the `bottom_hints` vec)
3. Update help text in `ui/mod.rs::render_help()`
4. Update README.md keybindings table

### New dialog
1. Create `src/dialog/<name>.rs` with a struct implementing `Dialog` (payload + cursor/scroll state, `render`, `handle_key`, optional `background`).
2. Register the module in `src/dialog/mod.rs`.
3. Open from input handlers with `app.open_dialog(Box::new(MyDialog { ... }))`.
4. If the dialog needs side effects beyond `Command` (hard delete, board create) add a variant to `DialogSideEffect` and handle in `input/dialog_input.rs::apply_side_effect`.

### New card field
1. Add field to `Card` struct in `model/card.rs` (with serde attributes)
2. Add `CardDetailTab` variant in `app.rs` if it needs its own tab
3. Add render function in `ui/card_detail.rs`
4. Add input handling in `input/card_detail_input.rs`
5. If editable via insert: add a new `InsertHandler` struct in the appropriate widget-kind submodule under `src/insert/`.

### New insert handler
1. Pick the right widget kind: `src/insert/line_editor.rs` (single-line text), `src/insert/markdown_editor.rs` (multi-line markdown), `src/insert/date_picker.rs`, or add a new submodule.
2. Add a struct holding its payload + the shared base (`LineInput`, `TextAreaInput`).
3. Implement `InsertHandler { handle_key, surface, title?, line_buffer?, line_cursor?, as_any }`.
4. Open with `app.start_insert(Box::new(MyHandler::new(...)))` from the relevant input handler.
5. The rendering site (`ui/board_view.rs`, `ui/board_selector.rs`, `ui/card_detail.rs`) keys off `handler.surface()` and `handler.title()`/`line_buffer()` automatically — no UI edits needed for line editors.

### New CLI subcommand
1. Add a `<name>.rs` module in `src/cli/` with a `pub(super) fn run`
2. Add `mod <name>;` in `src/cli/mod.rs`
3. Wire the dispatch arm in `cli::run`'s `match sub`
4. Add the command's usage block to the `HELP` string in `cli/mod.rs`
5. Update README.md CLI section

## Keep in Sync

When changing keybindings or features, update ALL of:
- `src/ui/mod.rs` — help overlay text
- `src/ui/status_bar.rs` — mode hint strings
- `src/ui/card_detail.rs` — bottom hint spans (for card detail modes)
- `README.md` — keybindings tables
- This file if architectural patterns change

## Key Patterns

- `has_ctrl_or_cmd()` in `insert/line_input.rs` — checks both Ctrl and Cmd (macOS Super) modifiers
- `LoadedBoard` in `app.rs` — holds all board state including selection indices
- `app.start_insert(Box::new(handler))` — enter Insert mode with a fresh `InsertHandler`
- `app.open_dialog(Box::new(dialog))` / `app.close_dialog()` / `app.close_dialog_to(mode)` — manage the modal `Dialog` stack
- `board.current_card()` / `board.current_card_id()` — get currently selected card
- `card.touch()` — updates `updated_at` timestamp
- `board.clamp_selection()` — fix selection indices after card removal
- `markdown::WRAP_WIDTH` — word-wrap width for description (80 chars)
- `markdown::build_visual_map()` / `markdown::source_to_visual()` — map source cursor ↔ visual (wrapped) line position

## Agent skills

### Issue tracker

Local markdown under `.scratch/<feature>/`. See `docs/agents/issue-tracker.md`.

### Triage labels

Canonical names (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context (`CONTEXT.md` + `docs/adr/` at root). See `docs/agents/domain.md`.
