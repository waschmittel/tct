# tct — Terminal Card Tracker

A keyboard-driven TUI Kanban board built in Rust. Think Trello, but in your terminal.

tct stores everything as **plain JSON files** — one file per board and one per card (lists live inline in the board file). Put them in a git repo, Dropbox, or any synced folder and every team member sees the same boards. tct watches the filesystem and picks up changes automatically, making it a natural fit for **background file sync** workflows.

![demo](docs/vhs/demo.gif)

## Why tct?

- **Files are the API** — every board and card is a standalone JSON file. Human-readable, git-friendly, scriptable.
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
- Per-board configurable accent color (pastel palette cycle with 'c', or free HSL color picker with 'C')
- Periodic filesystem reload (every 15s) for background sync support
- Full CLI interface for scripting and AI agent use (`tct --help`)

## Installation

Download a prebuilt binary for your platform from the
[GitHub releases page](https://github.com/waschmittel/tct/releases), then put it
on your `PATH`:

```sh
chmod +x tct
mv tct /usr/local/bin/
```

### Build from source

Requires a Rust toolchain.

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
      board.json          # Board metadata + ordered list definitions + labels
      card-<id>.json      # Card data: list_id, position, title, description, labels, ...
```

Each card owns its own list membership (`list_id`) and order within that list
(`position`, a fractional rank). Lists are just named entries in `board.json` —
there are no `list-*.json` files. This keeps a card's archived flag and its list
membership in a single file, so they cannot diverge.

All writes are atomic (write to `.tmp`, then rename) — no partial files, even on
crash. Boards created by older versions (with `list-<id>.json` files) are
migrated to this layout automatically the first time they are opened.

### Setting up shared boards with git

```sh
mkdir .tct
echo ".tct/**/*.tmp" >> .gitignore
tct boards --create "Sprint Board"
git add .tct/ && git commit -m "Add project board"
```

Team members who pull will see the board when they run `tct` from that project directory. Different cards never conflict. tct auto-reloads from disk every 15 seconds.

For detailed sync workflows (git, Dropbox, Syncthing), see the [User Guide](docs/user-guide.md).

All keybindings are discoverable in the TUI via the `?` help overlay, which is contextual to the current mode.

## CLI Usage

tct can be used as a CLI tool without opening the TUI — useful for scripting or AI agent integration.

```
tct --help                              Show all commands and options
tct --version                           Print the version and exit
tct --board <name>                      Open TUI directly on a matching board
```

Commands use the pattern `tct <entity> <board> --<action> [args]`. Board (and card for checklist) always come before the action flag. The default action for each entity is listing.

Name arguments use **case-insensitive partial matching** by default. Pass `--by-id` anywhere in the command to match all identifier arguments by **exact ID** instead of name. IDs are shown in listings as `[xxxxxxxx]`. Multiple name matches produce an error listing all candidates with their IDs.

Entities: `boards`, `lists`, `cards`, `checklist`, `labels`, `search`. Run `tct <entity> --help` (or `tct --help`) for the full set of actions and flags on each.

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
- **[Architecture (source tree)](docs/architecture.md)** — module layout and modal input dispatch overview

## License

MIT
