# Architecture

Source tree overview. For system context, runtime views, and quality scenarios
see the [arc42 document](arc42.md); for design decisions see [docs/adr/](adr/).

```
src/
  main.rs            # Entry point, CLI dispatch, TUI event loop
  app.rs             # App state, AppMode, LoadedBoard, on_tick reload
  board_editor.rs    # Aggregate root for the open board (ADR-0001/0002)
  board_directory.rs # Board collection: create/archive/rename/order (ADR-0004)
  command.rs         # Command enum — sole chokepoint for domain mutations (ADR-0002)
  event.rs           # Crossterm event polling (key + tick)
  term_caps.rs       # Terminal capability detection + color/glyph degradation
  cli/
    mod.rs           # CLI dispatch, help text
    boards.rs        # `tct boards`
    lists.rs         # `tct lists`
    cards.rs         # `tct cards`
    checklist.rs     # `tct checklist`
    labels.rs        # `tct labels`
    search.rs        # `tct search`
    lookup.rs        # ID/name resolution shared by all subcommands
    util.rs          # Flag parsing + output formatting helpers
  model/
    board.rs         # BoardMeta, ListMeta (lists live inline in board.json)
    card.rs          # Card (list_id, position), ChecklistItem, history
    ids.rs           # ShortId generation
    label.rs         # Label, LabelColor (pastel generation)
    list.rs          # CardList (derived view) + build_lists/ordered_card_ids
  storage/
    mod.rs           # StorageError, atomic_write
    paths.rs         # Data-dir resolution + path helpers
    board_store.rs   # Board CRUD (triggers migration on load)
    card_store.rs    # Card CRUD + load_all_cards + archived listing
    migrate.rs       # One-time legacy list-*.json -> card-owned migration
    list_store.rs    # Legacy/test-only (#[cfg(test)]) — see ADR-0006
  input/
    mod.rs           # Input dispatch by AppMode
    keymap.rs        # Generic Binding table + lookup + help_rows (ADR-0005)
    normal.rs        # Board view keybindings
    card_detail_input.rs   # Card detail keybindings
    board_selector_input.rs# Board selector keybindings
    insert.rs        # Insert-mode dispatch over the active InsertHandler
    dialog_input.rs  # Dialog-mode dispatch + side effects
    command.rs       # `:` / search command-mode input
  insert/            # InsertHandler trait, one struct per target (ADR-0003)
    mod.rs           # InsertHandler trait + InsertOutcome
    line_editor.rs   # Single-line text inputs (share LineInput)
    markdown_editor.rs # Description editor (over TextAreaInput): autocontinue, nest
    date_picker.rs   # Due-date picker
    line_input.rs    # Shared single-line buffer/cursor base
    text_area_input.rs # Shared multi-line buffer base
  dialog/            # Dialog trait, one struct per dialog kind (ADR-0003)
    mod.rs           # Dialog trait + DialogOutcome
    common.rs        # Shared dialog helpers
    confirm_*.rs     # Archive/cancel-edit/delete-label confirmations
    label_picker.rs  # Assign/remove labels on a card
    label_manager.rs # Board label CRUD
    archived_*.rs    # Archived boards/lists/cards browsers
    card_history.rs  # Card history viewer
    color_picker.rs  # Free HSL accent/label color picker
  ui/
    mod.rs           # Render dispatch + help overlay (reads keymap tables)
    board_view.rs    # Board columns layout
    board_selector.rs# Board list screen
    card_detail.rs   # Card detail overlay + description editor renderer
    dialog.rs        # Dialog frame helpers
    search_bar.rs    # Search input bar
    status_bar.rs    # Mode + hints + status messages
    markdown.rs      # Markdown rendering, word-wrap, source<->visual cursor map
    theme.rs         # Color theme
    snapshot_tests.rs# Golden-screen tests (insta + TestBackend)
    widgets/
      card_widget.rs # Individual card rendering
      list_widget.rs # List column rendering
      date_picker.rs # Calendar grid for the due-date picker
```

Modal input dispatch: the `AppMode` enum (`app.rs`) selects the active input
handler. `Insert` and `Dialog` are parameterless — the live handler is a
`Box<dyn InsertHandler>` / `Box<dyn Dialog>` on `App` (ADR-0003). Other modes
dispatch through per-mode keymap tables (ADR-0005).
