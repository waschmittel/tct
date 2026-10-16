# CLAUDE.md — Agent Instructions for tct

## Build & Test

```sh
cargo build          # Must compile with zero errors and zero warnings
cargo test -- --test-threads=1   # Tests use shared filesystem state
```

- **Golden-screen tests**: `src/ui/snapshot_tests.rs` renders the full UI into a ratatui `TestBackend` and snapshots the text with `insta` (goldens in `src/ui/snapshots/`). After intentional UI changes: `INSTA_UPDATE=always cargo test snapshot_ -- --test-threads=1`, inspect the `.snap` diff, commit. Fixture rule: no due dates / history timestamps (rendered relative to now → churn).
- **vhs demo + visual regression**: `docs/vhs/*.tape` drive the real binary (seeded by `docs/vhs/seed.sh`); `demo.tape` produces `docs/vhs/demo.gif` + a final-frame text golden, `linux-tty.tape` runs with `TERM=linux` to cover the degraded rendering tier and the char-alias card moves (vhs cannot send Shift+Arrow). check.sh trims vhs's multi-frame .txt to its last frame — intermediate frames race against status-message timing. `./docs/vhs/check.sh` re-records and diffs all tapes (requires `brew install vhs`; goldens are platform/font dependent — regenerate locally, don't compare cross-platform).

## Architecture

- **Modal input**: `AppMode` enum in `app.rs` drives which handler in `input/` runs. Modes: BoardSelector, Normal, CardDetail, Insert, Command, Dialog, Help. `Insert` and `Dialog` are parameterless — the active handler is `Box<dyn>` on `App.insert` / `App.dialog`. BoardSelector/Normal/CardDetail dispatch via per-mode `KEYMAP` tables (see "New keybinding").
- **Base vs overlay modes**: BoardSelector/Normal/CardDetail are *base* modes (`AppMode::is_base()`); Insert/Command/Dialog/Help are transient overlays. The overlay return target (`App.previous_mode`) may only ever hold a base mode — record via `app.remember_return_mode()`, restore via `app.take_return_mode()`; never write/consume `previous_mode` directly (an overlay recorded as return target once froze the UI: mode `Insert` with no handler swallows every key). `handle_input` calls `app.enforce_mode_invariants()` after every event — panics in debug/test, self-heals in release.
- **Board Editor** (`src/board_editor.rs`, ADR-0001/0002): aggregate root for the open board. `App.editor: Option<BoardEditor>`; reads via `app.board() -> Option<&LoadedBoard>`; domain mutations via `app.apply(Command)`; selection moves via editor verbs (`select_card_down`, `detail_item_up`, `reset_detail_cursor`, …). Input/UI code never mutates `LoadedBoard` fields directly.
- **Board Directory** (`src/board_directory.rs`): owns the board collection — create/archive/restore/rename/cycle accent/display order/listing. Used by the board selector, insert/dialog side effects, and `tct boards`. Stores are internals of the two aggregates.
- **Visibility**: `LoadedBoard::visible_cards(list_idx, search)` is the single source of truth for which Cards show (archived always hidden; search hides non-matching). Navigation, clamping, and rendering all consume it.
- **Dialog trait** (`src/dialog/`): One struct per dialog kind implementing `Dialog { render, handle_key, background, help }`. Each holds its own payload + cursor/scroll state. Returns `DialogOutcome { apply: Option<Command>, side_effect, status, follow }`. The dispatcher in `input/dialog_input.rs` interprets the outcome. See `docs/adr/0003-dialog-and-insert-as-traits.md`. Every dialog implements `help()` (`DialogHelp` rows for the Help overlay) and maps `?` to `DialogOutcome::help()` — `Follow::Help` shows the overlay while the dialog stays alive; Help-Esc returns to it.
- **InsertHandler trait** (`src/insert/`): One struct per insert target, grouped by widget kind: `line_editor.rs` (single-line inputs sharing `LineInput`), `markdown_editor.rs` (description editor over `TextAreaInput`), `date_picker.rs`. Returns `InsertOutcome` (`Stay`, `Cancel`, `Confirm(Command)`, `ConfirmAndOpenDialog`, `OpenDialog`, …). Dispatcher in `input/insert.rs`.
- **Storage**: JSON files under `~/.tct/boards/` (override with `TCT_DATA_DIR`). One file per board (`board.json`) and per card (`card-<id>.json`) — the path carries the id, which is not repeated inside the file (ADR-0007); **no list files** — lists are inline ordered `lists: [ListMeta]` in `board.json`. A Card owns membership (`list_id`) + order (`position`, fractional rank) — ADR-0006; `CardList.card_ids` is *derived* (`model/list.rs`), never persisted. Boards carry their own display order (`position`, fractional rank — ADR-0008); there is no order file. All writes via `atomic_write` (`.tmp` then rename).
- **Format version** (`src/storage/version.rs`, ADR-0009): `.tct/.version` holds one integer, checked once by `board_store::open_data_dir` — which every entry point already calls, so the gate cannot be skipped. Newer than `version::CURRENT` is refused; older is refused too, and that arm is the *only* place a future migration may live. There are no on-load migrations: the loaders parse exactly one shape, no legacy field lingers on a domain type, and no `Value` munging happens on the read path. When you change the persisted format, bump `CURRENT`.
- **Untrusted data dir**: `paths::base_dir()` walks up from cwd and adopts any `.tct` it finds, so the JSON may come from a checked-out repository — treat every file under the data dir as attacker-controlled. Three invariants keep that safe, the first two enforced by types rather than by checks at call sites:
  - **Ids are typed and validated** (`model/id.rs`): one newtype per entity — `BoardId`, `CardId`, `ListId`, `LabelId` — generated by a shared macro, each with a `Deserialize` enforcing `[A-Za-z0-9_-]{0,64}`. A `..` or `/` in an id cannot exist. `storage::paths` accepts only `BoardId`/`CardId`, the two that *are* locations; `ListId`/`LabelId` are references into `board.json` and cannot build a path, and swapped arguments are a compile error. Never widen the charset, never collapse the four back into one type, never add a `From<String>`/`&str` conversion outside `#[cfg(test)]`, and never build a data-dir path from a bare `&str`.
  - **The path is the id** (ADR-0007) — a Board's id is its directory, a Card's id is its file name; neither is persisted (`#[serde(skip)]` on `Card.id`/`BoardMeta.id`). `board_store::read_board_at` and `card_store::load_card` are the *only* places these types are deserialized and they stamp the id from the path, so read path and write path cannot diverge and no file can shadow another record. Don't add a third parse site; `save_card`/`save_board` `debug_assert!` the id was stamped. Directory/file names that are not valid ids are not boards/cards and are passed over.
  - **Nothing under the data dir may be a symlink** — validated ids keep a *path* inside the data dir, but a symlink redirects in the filesystem, where the path cannot show it. Every read goes through `storage::read_unlinked`; the boards dir is checked with `reject_symlink`, symlinked board dirs are skipped (`DirEntry::file_type` does not follow — don't "fix" it to `entry.path().is_dir()`), and a *discovered* `.tct` must be a real directory (`TCT_DATA_DIR` and `~/.tct` may be symlinks — those are the user's own choice, not repository content).
- **CLI output** (`src/term_out.rs`): card titles, list names and file names reach the terminal verbatim in CLI mode, so `cli/` and `storage/` must print through `outln!`/`errln!`, never `println!`/`eprintln!` — the macros strip control characters from the whole formatted line (keeping `\n`/`\t`), closing the ANSI-escape injection channel. The TUI needs no equivalent: ratatui already drops control chars when filling its cell buffer.
- **Description editing**: Lives on the `MarkdownEditor` insert handler (`src/insert/markdown_editor.rs`). Wraps `tui-textarea-2::TextArea` via `TextAreaInput` shared base; the textarea itself handles soft wrap (`WrapMode::WordOrGlyph`), wrapped Up/Down navigation, scrolling, and selection — `card_detail.rs::render_description_editor()` renders the widget frameless inside the detail popup (popup title gains "(editing description)"; text width capped at `WRAP_WIDTH`) plus a cursor-tracking scrollbar. List autocontinue + renumbering + nest/unnest live on the handler. While editing, the buffer shows as plain text; the styled markdown preview is the read view (scrollbar when it overflows, PgUp/PgDn scroll one line).
- **Markdown rendering**: `MarkdownRenderer` in `ui/markdown.rs`, word-wrap at `WRAP_WIDTH` (80). Read-view display only (`render() -> Rendered`, `.lines()`); the editor does not use it.
- **Label colors**: `LabelColor` enum with named pastel variants + `Custom { r, g, b }`. New labels get auto-generated pastel colors via `LabelColor::generate_pastel()` which picks maximally distant hue from existing labels.
- **Board accent color**: Each board has an `accent_color: LabelColor` field in `BoardMeta`. All UI highlight/accent colors use `app.accent_color()` instead of hardcoded `Color::Cyan`. New boards auto-get a differentiated pastel color. Users cycle with 'c' in board selector. Help overlay keeps structural Cyan.
- **Terminal capabilities** (`src/term_caps.rs`): `App.caps` holds the detected `TermCaps` (set once in `main`; `TermCaps::full()` everywhere else so tests are environment-independent). `ui::render` post-processes the finished buffer — RGB colors quantize to 256 colors when `COLORTERM` doesn't advertise truecolor, and to the 16 ANSI colors on `TERM=linux`, which also swaps glyphs missing from console fonts (`selected_border_type()`, `check_mark()`, `selection_bg()`). Style sites stay tier-unaware; consult `app.caps` only where the quantized style would be unusable. `main` also forces `terminal.clear()` after init — the Linux console has no alternate screen buffer, so without it stale shell output shines through "empty" cells. Because the console never transmits modifiers on arrow keys either, shift-free alternatives exist: grab mode (`m` in the board view — `App.grab_active`, arrows move the card, `m`/Esc drop, MOVE status chip) and `,`/`.` for the date-picker year jump.
- **Status messages**: Transient feedback ("Card archived", …) renders as a yellow toast in the *top-right corner* (`ui/mod.rs::render_status_toast`) — not in the status bar, so the `?:help` hint stays visible. Set via `app.set_status(msg)`; cleared by `App::on_tick()` after 30s. The status bar (`ui/status_bar.rs`) carries only the mode chip, the single hint, and the data dir (right-aligned).
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
4. Implement `help()` (its `DialogHelp` rows document all the dialog's keys) and map `?` to `DialogOutcome::help()` in `handle_key` — every dialog does this; there are no inline hints.
5. If the dialog needs side effects beyond `Command` (hard delete, board create) add a variant to `DialogSideEffect` and handle in `input/dialog_input.rs::apply_side_effect`.

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
- The matching help page: `Dialog::help()` rows for dialogs, the date-picker/description-editor pages in `ui/mod.rs::render_help()` for insert handlers
- This file if architectural patterns change

The status bar carries a single `?:help` hint (plus the insert title); there are **no other inline shortcut hints** anywhere — all shortcuts live in the contextual help overlay only. Status messages never occupy the status bar; they toast top-right (see "Status messages" above).

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

## Agent skills

### Issue tracker

Local markdown under `.scratch/<feature>/`. See `docs/agents/issue-tracker.md`.

### Triage labels

Canonical names (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: `UBIQUITOUS_LANGUAGE.md` (domain glossary) + `docs/adr/` at root. See `docs/agents/domain.md`.
