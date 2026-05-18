# tct — Terminal Card Tracker

A keyboard-driven TUI Kanban board built in Rust. Think Trello, but in your terminal.

tct stores everything as **plain JSON files** — one file per card, list, and board. Put them in a git repo, Dropbox, or any synced folder and every team member sees the same boards. tct watches the filesystem and picks up changes automatically, making it a natural fit for **background file sync** workflows.

## Why tct?

- **Files are the API** — every entity is a standalone JSON file. Human-readable, git-friendly, scriptable.
- **Sync-friendly by default** — one file per entity, atomic writes, periodic reload. Works with git, Dropbox, Syncthing, or any file sync tool.
- **Keyboard-first** — every action reachable via keyboard. Modal input (like Vim) keeps bindings contextual and safe.
- **Dual interface** — interactive TUI for humans, headless CLI for scripts and AI agents. Same data.
- **Zero infrastructure** — single binary. No database. No server. No account.

## Features

- Multiple boards with named lists and cards, reorderable and archivable
- Shift+Arrow card movement (within and across lists)
- Inline markdown description editor with syntax highlighting and word-wrap (80 chars)
- Markdown syntax highlighting (code blocks, headings, bold/italic, inline code, lists)
- Unified card detail view — description, checklist, labels, due date in one scrollable view
- Checklist CRUD — add, edit, toggle, delete, reorder items
- Board-level global labels with auto-generated pastel colors
- Due dates with overdueness display in both card list and detail view
- Description indicator (`≡`) in card list when a card has a description
- Card and board archiving with restore and permanent delete
- Search with non-matching cards hidden and label filtering
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

## Storage and Sync

Data is stored as JSON files. Storage location is resolved in this order:

1. `TCT_DATA_DIR` environment variable (if set)
2. A `.tct/` directory in the current working directory or any of its parents (project-local boards)
3. `~/.tct/` (global default)

This means you can keep project-specific boards alongside your code by creating a `.tct/` directory in your project root.

```
.tct/  (or ~/.tct/)
  board_order.json        # Board display order
  boards/
    <board-id>/
      board.json          # Board metadata + list order + labels
      list-<id>.json      # List name + card ID order
      card-<id>.json      # Card data (title, description, labels, etc.)
```

All writes are atomic (write to `.tmp`, then rename) — no partial files, even on crash.

### Setting up shared boards with git

```sh
mkdir .tct
echo ".tct/**/*.tmp" >> .gitignore
tct boards --create "Sprint Board"
git add .tct/ && git commit -m "Add project board"
```

Team members who pull will see the board when they run `tct` from that project directory. Different cards never conflict. tct auto-reloads from disk every 15 seconds.

For detailed sync workflows (git, Dropbox, Syncthing), see the [User Guide](docs/user-guide.md).

## Keybindings

### Board Selector

| Key | Action |
| --- | --- |
| Up/Down | Navigate boards |
| Shift+Up/Down | Reorder board up/down |
| Enter | Open board |
| n | New board |
| c | Cycle board accent color |
| d | Archive board |
| v | View/restore archived boards |
| q | Quit |

### Board View (Normal Mode)

| Key | Action |
| --- | --- |
| Left/Right | Switch lists |
| Up/Down | Navigate cards |
| Shift+Left/Right | Move card to adjacent list |
| Shift+Up/Down | Move card up/down within list |
| g/G | First/last card |
| Enter | Open card detail |
| t | Quick-edit card title |
| e | Edit card description |
| n | New card |
| N | New list |
| r | Rename list |
| d | Delete card (confirm) |
| D | Delete list (confirm) |
| a | Archive card (confirm) |
| v | View/restore/delete archived cards |
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
| y | Copy description to clipboard |
| Y | Copy entire checklist as markdown to clipboard |
| Up/Down | Navigate checklist items |
| Shift+Up/Down | Reorder selected checklist item |
| Space | Toggle checklist item |
| a | Add checklist item |
| Enter | Edit selected checklist item |
| x | Delete selected checklist item |
| l | Assign/remove labels (label picker) |
| L | Manage labels (create, rename, color, reorder, delete) |
| u | Set due date |
| U | Clear due date |
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
| Up/Down | Move by visual (wrapped) line |
| Enter | Auto-continue list items |
| Esc | Cancel (confirms if changes exist) |

On macOS, Cmd can be used instead of Ctrl in terminals that support it (kitty, Alacritty, WezTerm).

## CLI Usage

tct can be used as a CLI tool without opening the TUI — useful for scripting or AI agent integration.

```
tct --help                              Show all commands and options
tct --board <name>                      Open TUI directly on a matching board
```

Commands use the pattern `tct <entity> <board> --<action> [args]`. Board (and card for checklist) always come before the action flag. The default action for each entity is listing.

Name arguments use **case-insensitive partial matching** by default. Pass `--by-id` anywhere in the command to match all identifier arguments by **exact ID** instead of name. IDs are shown in listings as `[xxxxxxxx]`. Multiple name matches produce an error listing all candidates with their IDs.

### Boards

```
tct boards                              List active boards with card counts
tct boards --archived                   List archived boards
tct boards --create <name>              Create a new board
tct boards --archive <name>             Archive a board
tct boards --restore <name>             Restore an archived board
tct boards --delete <name>              Permanently delete an archived board

# Use --by-id to address by exact ID:
tct boards --archive a1b2c3d4 --by-id
```

### Lists

```
tct lists <board>                         List all lists on a board
tct lists <board> --create <name>         Create a list
tct lists <board> --rename <list> <name>  Rename a list
tct lists <board> --delete <list>         Delete a list and all its cards
```

### Cards

```
tct cards <board>                            List all active cards grouped by list
tct cards <board> --list <list>              List active cards in a specific list
tct cards <board> --archived                 List archived cards
tct cards <board> --show <card>              Show full card detail
tct cards <board> --create <list> <title>    Create a card
tct cards <board> --edit <card> [flags]      Edit card fields
  --title <text>                               New title
  --description <text>                         New description (replaces existing)
  --due <YYYY-MM-DD|none>                      Set or clear due date
tct cards <board> --archive <card>           Archive a card
tct cards <board> --restore <card>           Restore an archived card to the first list
tct cards <board> --delete <card>            Permanently delete an archived card
```

### Checklist

```
tct checklist <board> <card>                  Show checklist
tct checklist <board> <card> --add <text>     Add a checklist item
tct checklist <board> <card> --toggle <n>     Toggle item n (1-based index)
tct checklist <board> <card> --delete <n>     Delete item n (1-based index)
```

### Labels

```
tct labels <board>                              List all labels
tct labels <board> --create <name>              Create a label
tct labels <board> --delete <label>             Delete a label (removes from all cards)
tct labels <board> --assign <card> <label>      Assign a label to a card
tct labels <board> --remove <card> <label>      Remove a label from a card
```

### Search

```
tct search <query>                      Search cards on all boards (case-insensitive substring)
tct search <query> --board <name>       Limit to boards matching name (flag is repeatable)
tct search <query> --list <name>        Limit to lists matching name
tct search <query> --regex              Treat query as a regular expression
tct search <query> --archived           Include archived cards in results
```

Searches match against card title, description, checklist item text, and label names.

### Examples

A typical workflow from scratch:

```sh
# Create a board and lists
tct boards --create "My Project"
tct lists "My Project" --create "To Do"
tct lists "My Project" --create "In Progress"
tct lists "My Project" --create "Done"

# Add cards
tct cards "My Project" --create "To Do" "Fix login bug"
tct cards "My Project" --create "To Do" "Write tests"

# View the board
tct cards "My Project"

# Edit a card
tct cards "My Project" --edit "Fix login" --title "Fix login bug" --due 2099-12-31

# Add checklist items
tct checklist "My Project" "Fix login" --add "Reproduce the bug"
tct checklist "My Project" "Fix login" --add "Write a failing test"
tct checklist "My Project" "Fix login" --toggle 1

# Create and assign labels
tct labels "My Project" --create "bug"
tct labels "My Project" --assign "Fix login" "bug"

# Archive a finished card
tct cards "My Project" --archive "Write tests"
tct cards "My Project" --archived
```

Addressing items by ID (use `--by-id` when name matching is ambiguous):

```sh
# List boards to find the ID
tct boards
# Active boards (1):
#   [a1b2c3d4]  My Project   3 lists, 1 active cards

# Use ID for unambiguous operations (--by-id applies to all identifier args)
tct lists a1b2c3d4 --by-id
tct cards a1b2c3d4 --show e5f6a7b8 --by-id
```

## Documentation

- **[User Guide](docs/user-guide.md)** — detailed usage, sync workflows, troubleshooting, data format reference
- **[Architecture (arc42)](docs/arc42.md)** — system context, building blocks, runtime views, ADRs, quality scenarios

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
