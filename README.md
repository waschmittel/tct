# tct — Terminal Card Tracker

A keyboard-driven TUI Kanban board built in Rust. Think Trello, but in your terminal.

## Features

- Multiple boards with named lists and cards, reorderable and archivable
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
- Per-board configurable accent color (pastel palette, cycle with 'c')
- Periodic filesystem reload (every 15s) for background sync support
- Full CLI interface for scripting and AI agent use (`tct --help`)

## Installation

```sh
cargo install --path .
```

Or build and run directly:

```sh
cargo run
```

## Storage

Data is stored as JSON files. Storage location is resolved in this order:

1. `TCT_DATA_DIR` environment variable (if set)
2. A `.tct/` directory in the current working directory or any of its parents (project-local boards)
3. `~/.tct/` (global default)

This means you can keep project-specific boards alongside your code by creating a `.tct/` directory in your project root.

```
.tct/  (or ~/.tct/)
  boards/
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
| J/K | Reorder board up/down |
| Enter | Open board |
| n | New board |
| c | Cycle board accent color |
| d | Archive board |
| v | View/restore archived boards |
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
| v | View/restore/delete archived cards |
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

## CLI Usage

tct can be used as a CLI tool without opening the TUI — useful for scripting or AI agent integration.

```
tct --help                              Show all commands and options
tct --board <name>                      Open TUI directly on a matching board
```

Board, list, card, and label arguments use **case-insensitive partial name matching** or **ID prefix matching** (IDs are shown in listings as `[xxxxxxxx]`). Multiple matches produce an error listing all candidates.

### Boards

```
tct boards                              List active boards with card counts
tct boards archived                     List archived boards
tct boards create <name>                Create a new board
tct boards archive <name>               Archive a board
tct boards restore <name>               Restore an archived board
tct boards delete <name>                Permanently delete an archived board
```

### Lists

```
tct lists <board>                       List all lists on a board
tct lists create <board> <name>         Create a list
tct lists rename <board> <list> <name>  Rename a list
tct lists delete <board> <list>         Delete a list and all its cards
```

### Cards

```
tct cards <board>                       List all active cards grouped by list
tct cards <board> <list>                List active cards in a specific list
tct cards archived <board>              List archived cards
tct cards show <board> <card>           Show full card detail
tct cards create <board> <list> <title> Create a card
tct cards edit <board> <card> [flags]   Edit card fields
  --title <text>                          New title
  --description <text>                    New description (replaces existing)
  --due <YYYY-MM-DD|none>                 Set or clear due date
tct cards archive <board> <card>        Archive a card
tct cards restore <board> <card>        Restore an archived card to the first list
tct cards delete <board> <card>         Permanently delete an archived card
```

### Checklist

```
tct checklist <board> <card>            Show checklist
tct checklist add <board> <card> <text> Add a checklist item
tct checklist toggle <board> <card> <n> Toggle item n (1-based index)
tct checklist delete <board> <card> <n> Delete item n (1-based index)
```

### Labels

```
tct labels <board>                         List all labels
tct labels create <board> <name>           Create a label
tct labels delete <board> <label>          Delete a label (removes from all cards)
tct labels assign <board> <card> <label>   Assign a label to a card
tct labels remove <board> <card> <label>   Remove a label from a card
```

## Architecture

```
src/
  main.rs          # Entry point, CLI dispatch, TUI event loop
  cli.rs           # All CLI subcommands (boards/lists/cards/checklist/labels)
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
