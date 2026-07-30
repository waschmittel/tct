# Board display order as a fractional rank

A Board carries its own place in the collection: `BoardMeta.position`, a fractional rank, exactly as a Card carries its place in a List (ADR-0006). `board_store::list_boards` sorts by `position`, breaking ties on `created_at` then id so the order is total and stable. There is no `board_order.json`.

## Why

Board order was the last central list in the storage layer, and it was the arrangement ADR-0006 rejected for Cards: a separate file naming the members of a collection whose membership is really the set of directories on disk. Stating membership twice means the two can disagree, and they did.

- A Board's *place* lived in `board_order.json`, its *existence* in `boards/<id>/`, and its *activeness* in both `board.json`'s `archived` flag **and** its presence in the order file — `archive` removed the id, `restore` appended it.
- Deleting a board directory left its id in the order file permanently. Verified before the change: create three boards, remove one directory, and the ghost id survived every later write. Harmless to rendering (unknown ids are skipped) but unbounded.

Moving the rank onto the Board removes the file, the drift, and the archive/restore coupling. Archiving is now a flag flip in one atomic write, like archiving a Card.

## Decisions

- **Reordering renumbers the displayed Boards rather than taking a midpoint.** Cards can always midpoint between neighbours because they have always had ranks. Boards written before this ADR all deserialize to `position: 0.0`, where a midpoint separates nothing. `board_directory::reorder` therefore assigns 1..n over the new on-screen order, writing only the Boards whose rank actually changes — after the first reorder the collection is separated and later moves touch two files. Boards number in the tens, so the one-time O(n) is not worth avoiding.
- **Each Board is re-read before its rank is written.** `reorder` takes the displayed `BoardMeta`s to compute the new order but loads each Board fresh before saving, so a rank write cannot clobber a concurrent edit to the same file made elsewhere (the periodic reload means the in-memory copy can be stale).
- **`position` is `#[serde(default)]`.** Boards written before this ADR read back at 0.0 and fall to creation order, which is stable and self-heals on the first reorder.
- **No migration from `board_order.json`.** Reading it once to seed ranks would be exactly the kind of one-shot legacy path ADR-0009 removes. A custom board order set before this change is lost; the file is left on disk, unread.

## Consequences

- `board_order.json`, `load_board_order`, `save_board_order`, `append_to_order` and `remove_from_order` are gone, as is `paths::board_order_path`.
- `board_directory::swap_order(a, b, displayed)` becomes `reorder(displayed, from, to)`, which states what the caller means (move this Board to that slot) rather than how it is stored.
- `model::rank` now holds the shared fractional-rank helpers (`between`, `after_last`, `cmp`), moved out of `model::list` where they were only nominally about Lists.
- Existing users lose their custom board order on upgrade and see creation order until they reorder once.
