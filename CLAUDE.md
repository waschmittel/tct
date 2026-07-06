# CLAUDE.md — Agent Instructions for tct

## Build & Test

```sh
cargo build          # Must compile with zero errors and zero warnings
cargo test -- --test-threads=1   # Tests use shared filesystem state
```

- **Golden-screen tests**: `src/ui/snapshot_tests.rs` renders the full UI into a ratatui `TestBackend` and snapshots the text with `insta` (goldens in `src/ui/snapshots/`). After intentional UI changes: `INSTA_UPDATE=always cargo test snapshot_ -- --test-threads=1`, inspect the `.snap` diff, commit. Fixture rule: no due dates / history timestamps (rendered relative to now → churn).
- **vhs demo + visual regression**: `docs/vhs/demo.tape` drives the real binary (seeded by `docs/vhs/seed.sh`) and produces `docs/vhs/demo.gif` + a final-frame text golden (check.sh trims vhs's multi-frame .txt to its last frame — intermediate frames race against status-message timing). `./docs/vhs/check.sh` re-records and diffs (requires `brew install vhs`; goldens are platform/font dependent — regenerate locally, don't compare cross-platform).

## Architecture

- **Modal input**: `AppMode` enum in `app.rs` drives which handler in `input/` runs. Modes: BoardSelector, Normal, CardDetail, Insert, Command, Dialog, Help. `Insert` and `Dialog` are parameterless — the active handler is `Box<dyn>` on `App.insert` / `App.dialog`. BoardSelector/Normal/CardDetail dispatch via per-mode `KEYMAP` tables (see "New keybinding").
- **Board Editor** (`src/board_editor.rs`, ADR-0001/0002): aggregate root for the open board. `App.editor: Option<BoardEditor>`; reads via `app.board() -> Option<&LoadedBoard>`; domain mutations via `app.apply(Command)`; selection moves via editor verbs (`select_card_down`, `detail_item_up`, `reset_detail_cursor`, …). Input/UI code never mutates `LoadedBoard` fields directly.
- **Board Directory** (`src/board_directory.rs`): owns the board collection — create/archive/restore/rename/cycle accent/display order/listing. Used by the board selector, insert/dialog side effects, and `tct boards`. Stores are internals of the two aggregates.
- **Visibility**: `LoadedBoard::visible_cards(list_idx, search)` is the single source of truth for which Cards show (archived always hidden; search hides non-matching). Navigation, clamping, and rendering all consume it.
- **Dialog trait** (`src/dialog/`): One struct per dialog kind implementing `Dialog { render, handle_key, background }`. Each holds its own payload + cursor/scroll state. Returns `DialogOutcome { apply: Option<Command>, side_effect, status, follow }`. The dispatcher in `input/dialog_input.rs` interprets the outcome. See `docs/adr/0003-dialog-and-insert-as-traits.md`.
- **InsertHandler trait** (`src/insert/`): One struct per insert target, grouped by widget kind: `line_editor.rs` (single-line inputs sharing `LineInput`), `markdown_editor.rs` (description editor over `TextAreaInput`), `date_picker.rs`. Returns `InsertOutcome` (`Stay`, `Cancel`, `Confirm(Command)`, `ConfirmAndOpenDialog`, `OpenDialog`, …). Dispatcher in `input/insert.rs`.
- **Storage**: JSON files under `~/.tct/boards/` (override with `TCT_DATA_DIR`). One file per board (`board.json`) and per card (`card-<id>.json`); **no list files** — lists are inline ordered `lists: [ListMeta]` in `board.json`. A Card owns membership (`list_id`) + order (`position`, fractional rank) — ADR-0006; `CardList.card_ids` is *derived* (`model/list.rs`), never persisted. All writes via `atomic_write` (`.tmp` then rename). Legacy `list-*.json` boards migrate on load (`storage/migrate.rs` via `board_store::load_board`); `list_store` is now `#[cfg(test)]`.
- **Description editing**: Lives on the `MarkdownEditor` insert handler (`src/insert/markdown_editor.rs`). Wraps `ratatui-textarea::TextArea` via `TextAreaInput` shared base. List autocontinue + renumbering + nest/unnest also live there. Renderer in `card_detail.rs::render_description_editor()` reads from the handler.
- **Markdown rendering**: `MarkdownRenderer` in `ui/markdown.rs`, word-wrap at `WRAP_WIDTH` (80). `render() -> Rendered` owns the source↔visual cursor mapping (see Key Patterns); one wrap impl serves both rendering and cursor movement.
- **Label colors**: `LabelColor` enum with named pastel variants + `Custom { r, g, b }`. New labels get auto-generated pastel colors via `LabelColor::generate_pastel()` which picks maximally distant hue from existing labels.
- **Board accent color**: Each board has an `accent_color: LabelColor` field in `BoardMeta`. All UI highlight/accent colors use `app.accent_color()` instead of hardcoded `Color::Cyan`. New boards auto-get a differentiated pastel color. Users cycle with 'c' in board selector. Help overlay keeps structural Cyan.
- **Search**: When active, non-matching cards are hidden (not just dimmed). Navigation skips hidden cards. First match auto-selected on search confirm.
- **Periodic reload**: `App::on_tick()` reloads board from filesystem every 15s (configurable via `reload_interval`). Skipped during editing/dialog/grab modes. Preserves selection state.

## How to Add Things

### New keybinding (board selector / board view / card detail)
Keybindings live in a **keymap table** per mode (`KEYMAP` in `input/board_selector_input.rs`, `input/normal.rs`, `input/card_detail_input.rs`). Dispatch and the help overlay both read the table.
1. Add an `Action` enum variant and a `Binding` row to the mode's `KEYMAP` (key, action, help text, section)
2. Add the action's match arm in the same file's `run()`
3. The help overlay (`ui/mod.rs::render_help()`) generates rows from the table — only touch it if you introduce a new *section* (and extend the section list + the `help_layout_covers_all_keymap_sections` test)

The help overlay is the single source of truth for keybindings — README.md no longer lists them.

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
4. Add the command's usage block to the `HELP` string in `cli/mod.rs` (this is the full CLI reference — README.md only carries basic usage + an example workflow)

## Keep in Sync

When changing keybindings or features, update:
- The mode's `KEYMAP` table (help overlay generates from it)
- `src/ui/status_bar.rs` mode hints; `src/ui/card_detail.rs` bottom hint spans
- This file if architectural patterns change

Doc homes (don't duplicate elsewhere): keybindings → help overlay only; full CLI reference → `cli/mod.rs` `HELP`; module layout → `docs/architecture.md`; README keeps basic usage + one example workflow.

## Key Patterns

- `has_ctrl_or_cmd()` in `insert/line_input.rs` — checks both Ctrl and Cmd (macOS Super) modifiers
- `LoadedBoard` in `app.rs` — holds all board state including selection indices
- `app.start_insert(Box::new(handler))` — enter Insert mode with a fresh `InsertHandler`
- `app.open_dialog(Box::new(dialog))` / `app.close_dialog()` / `app.close_dialog_to(mode)` — manage the modal `Dialog` stack
- `board.current_card()` / `board.current_card_id()` — get currently selected card
- `card.touch()` — updates `updated_at` timestamp
- `board.clamp_selection()` — fix selection indices after card removal
- `markdown::WRAP_WIDTH` — word-wrap width for description (80 chars)
- Source↔visual cursor mapping: `MarkdownRenderer::render() -> Rendered`, then `Rendered::{cursor_at, source_pos_at, src_row_for, visual_line_count}`

## Agent skills

### Issue tracker

Local markdown under `.scratch/<feature>/`. See `docs/agents/issue-tracker.md`.

### Triage labels

Canonical names (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: `UBIQUITOUS_LANGUAGE.md` (domain glossary) + `docs/adr/` at root. See `docs/agents/domain.md`.
