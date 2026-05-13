# tct — Terminal Card Tracker

A keyboard-driven TUI Kanban board built in Rust. Think Trello, but in your terminal.

## Features

- Multiple boards with named lists and cards
- Grab-and-move card reordering (within and across lists)
- Inline markdown description editor with syntax highlighting
- Markdown syntax highlighting (code blocks, headings, bold/italic, inline code, lists)
- Unified card detail view — description, checklist, labels, due date in one scrollable view
- Checklist CRUD — add, edit, toggle, delete items
- Board-level global labels with auto-generated pastel colors
- Due dates with overdueness display in both card list and detail view
- Card archiving and un-archiving
- Search with non-matching cards hidden and label filtering
- Grab-and-move with confirm/abort (Esc restores card to original position)
- Confirmation dialogs for destructive actions
- Undo/redo in description editor
- macOS Cmd key support (in supported terminals)
- Auto-continuing lists in editor
- Periodic filesystem reload (every 15s) for background sync support

## Installation

```sh
cargo install --path .
```

Or build and run directly:

```sh
cargo run
```

## Storage

Data is stored in `~/.tct/boards/` as JSON files. Override with the `TCT_DATA_DIR` environment variable.

```
~/.tct/boards/
  <board-id>/
    board.json          # Board metadata + list order
    list-<id>.json      # List name + card ID order
    card-<id>.json      # Card data (title, description, labels, etc.)
```

All writes are atomic (write to `.tmp`, then rename).

## Keybindings

### Board Selector

| Key | Action |
| --- | --- |
| j/k | Navigate boards |
| Enter | Open board |
| n | New board |
| d | Delete board |
| q | Quit |

### Board View (Normal Mode)

| Key | Action |
| --- | --- |
| h/l, Left/Right | Switch lists |
| j/k, Down/Up | Navigate cards |
| g/G | First/last card |
| e | Open card detail |
| Enter | Quick-edit card title |
| n | New card |
| N | New list |
| r | Rename list |
| d | Delete card (confirm) |
| D | Delete list (confirm) |
| a | Archive card (confirm) |
| v | View/restore archived cards |
| m | Grab card for moving |
| h/j/k/l (grabbed) | Move card |
| Enter (grabbed) | Confirm move |
| Esc (grabbed) | Abort — restore to original position |
| J/K | Reorder card up/down in list |
| </>  | Reorder list left/right |
| / | Search |
| L | Manage labels |
| F | Clear filters |
| b | Back to board selector |
| ? | Help |
| q | Quit |

### Card Detail

| Key | Action |
| --- | --- |
| t | Edit title |
| e | Edit description |
| j/k | Navigate checklist items |
| Space | Toggle checklist item |
| a | Add checklist item |
| Enter | Edit selected checklist item |
| x | Delete selected checklist item |
| l | Assign/remove labels (label picker) |
| L | Manage labels (create, rename, color, delete) |
| u | Set due date |
| Esc | Close |

### Description Editor

| Key | Action |
| --- | --- |
| Ctrl+S | Save |
| Ctrl+Z | Undo |
| Ctrl+Y | Redo |
| Ctrl+B | Bold (\*\*text\*\*) |
| Ctrl+I | Italic (\*text\*) |
| Ctrl+K | Inline code (\`text\`) |
| Ctrl+L | Insert list item (- ) |
| Enter | Auto-continue list items |
| Esc | Cancel (confirms if changes exist) |

On macOS, Cmd can be used instead of Ctrl in terminals that support it (kitty, alacritty, WezTerm).

## Architecture

```
src/
  main.rs          # Event loop, terminal setup
  app.rs           # App state, modes, board loading
  model/
    board.rs       # BoardMeta
    card.rs        # Card, ChecklistItem
    ids.rs         # ShortId generation
    label.rs       # Label, LabelColor
    list.rs        # CardList
  storage/
    mod.rs         # StorageError, atomic_write
    paths.rs       # Path helpers
    board_store.rs # Board CRUD
    list_store.rs  # List CRUD
    card_store.rs  # Card CRUD + archived listing
  input/
    mod.rs         # Input dispatch by mode
    normal.rs      # Board view keybindings
    insert.rs      # Text input + description editor
    card_detail_input.rs  # Card detail keybindings
    dialog_input.rs       # Dialog handlers
    board_selector_input.rs  # Board selector keybindings
  ui/
    mod.rs         # Render dispatch + help overlay
    board_view.rs  # Board columns layout
    board_selector.rs  # Board list screen
    card_detail.rs     # Card detail overlay + editor renderer
    dialog.rs          # Confirmation + picker dialogs
    search_bar.rs      # Search input bar
    status_bar.rs      # Mode + hints + status messages
    markdown.rs        # Markdown rendering, syntax highlighting
    theme.rs           # Color theme
    widgets/
      card_widget.rs   # Individual card rendering
      list_widget.rs   # List column rendering
```

Modal input dispatch: `AppMode` enum determines which input handler processes keys. `InsertTarget` tracks what's being edited. `DialogKind` tracks which dialog is showing.

## License

MIT
