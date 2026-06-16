# Card-owned list membership

A Card stores its own list membership and order: `card.list_id` names the List it belongs to, and `card.position` (a fractional rank) orders it within that List. Lists are defined inline in `board.json` as an ordered `lists: [ListMeta { id, name, archived }]`. There are no `list-*.json` files; a List's cards are derived by scanning `card-*.json` for matching `list_id` and sorting by `position` (`model/list.rs::build_lists`/`ordered_card_ids`).

This supersedes the storage aspect of the layout described in [0001](0001-board-editor-aggregate.md), where each List lived in its own file carrying a `card_ids` array and `board.json` held a `list_order`.

## Why

Membership lived in two files: a Card's `archived` flag was in `card-<id>.json`, but whether the Card belonged to a List was in that List's `card_ids` array. Writes across files are not atomic (`commit_pending` flushes each file separately — see 0001's rejected write-ahead journal). The dangerous case was `RestoreCard`: it wrote the card (`archived = false`) first, then appended the id to the list. A crash, IO error, or partial flush in between left a card that was **not archived** and **referenced by no list** — invisible in the board view, and excluded from the archived-cards view (which scans for `archived == true`). The card was fully persisted but unreachable from any UI.

Co-locating the archived flag and the membership pointer in the single `card-*.json` file removes the cross-file invariant entirely: archive/restore is a one-field flip in one atomic write, and a non-archived card can no longer be orphaned because its home is intrinsic to it.

## Decisions

- **Fractional `position` (f64), not integer reindex.** A move/insert sets the moved card's `position` to the midpoint of its new neighbors, so reordering rewrites exactly one card file. Integer indices would force rewriting every card in the list on each move. Precision is not a practical concern for a single-user TUI; positions can be renormalized on load if ever needed.
- **List defs in `board.json`, including archived lists.** `ListMeta.archived` flags an archived List; its cards keep their `list_id`, so restoring a List is a flag flip and the cards reappear in place. The active board view filters `archived == false`.
- **Archived cards keep their `list_id` and stay in the in-memory `card_ids`.** `LoadedBoard::visible_cards` remains the single source of visibility (it filters the `archived` flag), so navigation/rendering are unchanged.
- **Lazy label cleanup.** `DeleteLabel` only removes the board label; it does not rewrite every card to scrub dead `label_ids`. Dead ids are harmless in memory (`resolved_labels`/`matches_search` ignore unknown ids) and are pruned the next time each card is saved (`commit_pending`). This avoids an O(cards) write storm on label deletion.
- **Auto-migration on load.** `storage/migrate.rs::migrate_if_needed` runs from `board_store::load_board` (the single choke point for both TUI and CLI reads). If `list-*.json` files are present it stamps `list_id` + 1-based `position` onto each card, builds `meta.lists`, persists `board.json`, and deletes the list files. It is idempotent and cheap once migrated.

## Considered and rejected

- **Fold cards into `board.json` too (one file per board).** Would also make reorders single-file and remove all cross-file invariants, but loses the "one file per card" property that makes git diffs and conflict resolution clean (a core product value — see README). Rejected.
- **Keep `list-*.json` but add a load-time orphan reconciler.** Patches the symptom (re-home `archived == false` cards not in any list) without removing the divergent-state class. More code, weaker guarantee. Rejected in favor of making the bad state unrepresentable.
- **Write-ahead journal for cross-file atomicity.** Same reasoning as 0001 — infrastructure not earned for a single-user TUI. Card-owned membership sidesteps the need.

## Consequences

- `list_store` is now legacy/test-only (`#[cfg(test)]`); `paths::list_path` and `StorageError::ListNotFound` likewise. Migration reads legacy list files via raw `fs`, not `list_store`.
- New domain rule: a Card with `archived == false` must have a `list_id` naming an active List. `RestoreCard` enforces it by reattaching dangling cards to the first active List.
- A move writes one card file; add-card writes one card file; list add/rename/archive/restore/move write `board.json`. No more "card → list → board" save ordering for the common card paths.
- Card files gained `list_id` and `position` (both `#[serde(default)]`, so old files deserialize and get stamped on migration).
