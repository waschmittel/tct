# Board Directory owns the board collection

A **Board Directory** (`src/board_directory.rs`) owns every operation that spans the collection of **Boards** or runs before a board is loaded: create, archive/restore, rename, accent-color cycling, display order, listing (active + archived), and hard delete. The **Board Editor** (ADR-0001) keeps owning exactly one loaded board.

## Why

ADR-0001 closed the store-access leak for cards and lists, but board-collection verbs had no module: the board selector input handler, the `CreateBoard` insert side effect, and the `tct boards` CLI each called `board_store` directly and re-implemented rituals like create (generate pastel + save + append_to_order) and order-swap with backfill. Two adapters (TUI selector, CLI) justify the seam; concentrating the rituals gives locality for the rules and one interface to test headless.

## Considered and rejected

- **Extend Board Editor with collection verbs.** Rejected — the editor's invariant is "one loaded board"; collection operations would force it to exist without a board and blur the aggregate boundary.
- **A stateful `BoardDirectory` struct caching `Vec<BoardMeta>`.** Rejected — the collection's source of truth is the filesystem and `App.boards` already caches the display list. A stateless module of functions is the whole interface; a struct would be a pass-through (fails the deletion test).
- **Routing archive/restore/rename around the Command enum.** Rejected — these go through `BoardEditor::apply(Command)` internally so History/persistence rules stay at the single chokepoint (ADR-0002).

## Consequences

- `board_store` (like `card_store`/`list_store`) has no production callers outside the two aggregates; input handlers and CLI subcommands depend on `board_directory` / `BoardEditor` only.
- Archived card/list queries and the only hard deletes (archived card/list files) moved onto Board Editor as verbs (`archived_cards`, `delete_archived_card`, …).
- New board-collection features (e.g. board duplication) get one home and are immediately shared by TUI and CLI.
