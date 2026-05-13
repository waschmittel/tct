# CLAUDE.md — Agent Instructions for tct

## Build & Test

```sh
cargo build          # Must compile with zero errors and zero warnings
cargo test -- --test-threads=1   # Tests use shared filesystem state
```

## Architecture

- **Modal input**: `AppMode` enum in `app.rs` drives which handler in `input/` runs. Modes: BoardSelector, Normal, CardDetail, Insert(target), Command, Dialog(kind), Help.
- **InsertTarget**: Enum for what's being edited (card title, description, list name, checklist, due date). Each variant has input handling in `insert.rs` and rendering in `card_detail.rs`.
- **DialogKind**: Enum for dialogs (delete confirmations, archive, cancel edit, label picker, archived cards). Handlers in `dialog_input.rs`, rendering in `dialog.rs`.
- **Storage**: JSON files under `~/.tct/boards/`. All writes use `atomic_write` (write `.tmp`, then rename). Override path with `TCT_DATA_DIR` env var.
- **Description editing**: Uses `ratatui-textarea` TextArea for editing, custom renderer in `card_detail.rs::render_description_editor()` for syntax highlighting via `markdown::highlight_line()`.
- **Markdown rendering**: `pulldown-cmark` for read-only view (`markdown::render_markdown()`), hand-rolled line-level highlighter for editor (`markdown::highlight_line()`).
- **Table formatting**: `markdown::format_tables()` aligns columns on save.

## How to Add Things

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
1. Add variant to `DialogKind` in `app.rs`
2. Add handler function in `input/dialog_input.rs`, wire it in the `match kind` block
3. Add render arm in `ui/dialog.rs::render()`
4. Add to the `matches!` pattern in `ui/mod.rs::render()` so it renders on top

### New card field
1. Add field to `Card` struct in `model/card.rs` (with serde attributes)
2. Add `CardDetailTab` variant in `app.rs` if it needs its own tab
3. Add render function in `ui/card_detail.rs`
4. Add input handling in `input/card_detail_input.rs`
5. If editable via insert: add `InsertTarget` variant, handle in `input/insert.rs`

### New InsertTarget
1. Add variant to `InsertTarget` in `app.rs`
2. Handle in `cancel_insert()` and `confirm_insert()` in `input/insert.rs`
3. Add rendering (popup dialog or inline) in `ui/card_detail.rs`

## Keep in Sync

When changing keybindings or features, update ALL of:
- `src/ui/mod.rs` — help overlay text
- `src/ui/status_bar.rs` — mode hint strings
- `src/ui/card_detail.rs` — bottom hint spans (for card detail modes)
- `README.md` — keybindings tables
- This file if architectural patterns change

## Key Patterns

- `has_ctrl_or_cmd()` in `insert.rs` — checks both Ctrl and Cmd (macOS Super) modifiers
- `LoadedBoard` in `app.rs` — holds all board state including selection indices
- `app.start_insert()` / `app.start_insert_with()` — enter insert mode with optional pre-fill
- `app.start_description_edit()` — initializes TextArea + stores original for change detection
- `board.current_card()` / `board.current_card_id()` — get currently selected card
- `card.touch()` — updates `updated_at` timestamp
- `board.clamp_selection()` — fix selection indices after card removal
