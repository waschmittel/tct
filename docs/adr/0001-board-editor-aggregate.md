# Board Editor as aggregate root

A **Board Editor** (`src/board_editor.rs`) owns the **Loaded Board** + **Selection State** for the currently open board and is the only path through which board, list, and card mutations reach disk. It replaces direct calls into the three Stores from input handlers and CLI subcommands.

## Why

139 `*_store::save_*` call-sites and ~30 ad-hoc `card.log()` sites had spread persistence + History Entry orchestration across every input handler. Callers had to know save ordering (card → list.card_ids → list → board); forgetting a step silently corrupted data. Centralising this in an aggregate gives us locality for the rule and leverage for tests.

## Considered and rejected

- **Stateless service borrowing `&mut LoadedBoard` per call.** Rejected — leaves the write seam leaky; callers can still mutate `LoadedBoard` fields directly.
- **Write-ahead journal for cross-file atomicity.** Rejected for now — tct is a single-user TUI; the crash window is small and the journal infrastructure isn't earned. We use stage-all + commit-at-end instead: the editor buffers pending writes and flushes them on `commit()`. A single failed flush aborts the rest in-memory. Disk is still vulnerable to mid-flush crashes; that is the documented limitation.
- **Per-call error type as `anyhow::Result`.** Rejected for the interior — we use `thiserror::BoardEditorError` so callers can match on `NotFound` / `Invariant` / `Io`. Adapters (CLI argv handler, TUI input handler) convert to `anyhow` at the edge.

## Consequences

- `App.board: Option<LoadedBoard>` becomes `App.editor: Option<BoardEditor>`. UI reads via `editor.board() -> &LoadedBoard`.
- `LoadedBoard` fields move from `pub` to `pub(crate)`; mutations only via Board Editor verbs.
- Stores (`board_store`, `list_store`, `card_store`) become internals of `board_editor` and lose their public callers in `input/` and `cli/`.
