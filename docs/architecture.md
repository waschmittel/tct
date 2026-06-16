# Architecture

Source tree overview. For system context, runtime views, and ADRs see the
[arc42 document](arc42.md).

```
src/
  main.rs          # Entry point, CLI dispatch, TUI event loop
  app.rs           # App state, modes, board loading
  cli/
    mod.rs         # CLI dispatch, help text
    boards.rs      # `tct boards` subcommand
    lists.rs       # `tct lists` subcommand
    cards.rs       # `tct cards` subcommand
    checklist.rs   # `tct checklist` subcommand
    labels.rs      # `tct labels` subcommand
    search.rs      # `tct search` subcommand
    lookup.rs      # ID/name resolution shared by all subcommands
    util.rs        # Flag parsing + output formatting helpers
  model/
    board.rs       # BoardMeta, ListMeta
    card.rs        # Card (list_id, position), ChecklistItem
    ids.rs         # ShortId generation
    label.rs       # Label, LabelColor
    list.rs        # CardList (derived view) + build_lists/ordered_card_ids
  storage/
    mod.rs         # StorageError, atomic_write
    paths.rs       # Path helpers
    board_store.rs # Board CRUD (triggers migration on load)
    card_store.rs  # Card CRUD + load_all_cards + archived listing
    migrate.rs     # One-time legacy list-*.json -> card-owned migration
  input/
    mod.rs         # Input dispatch by mode
    normal.rs      # Board view keybindings
    insert/
      mod.rs           # Insert-mode dispatch + plain text-buffer editing
      description.rs   # Markdown description editor keybindings
      list_editing.rs  # List autocontinue, indent, renumber
      due_date.rs      # Due-date calendar picker
    card_detail_input.rs  # Card detail keybindings
    dialog_input.rs       # Dialog handlers
    board_selector_input.rs  # Board selector keybindings
    command.rs            # `:` command-mode input
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
      date_picker.rs   # Calendar grid for due-date picker
```

Modal input dispatch: `AppMode` enum determines which input handler processes keys. `InsertTarget` tracks what's being edited. `DialogKind` tracks which dialog is showing.
