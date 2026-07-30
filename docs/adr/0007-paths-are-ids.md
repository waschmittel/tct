# The path is the id

> Two claims below were overtaken by [ADR-0008](0008-board-order-as-rank.md) and [ADR-0009](0009-versioned-format-no-migrations.md): `board_order.json` and the legacy migration are gone, so **no** file content reaches a path component any more. Id validation is now defence in depth plus the filter that decides whether a directory or file name is a record at all. A hand-renamed board directory re-ids the board and falls to creation order, not alphabetical.

A Board's id is the directory holding its `board.json`; a Card's id is the `<id>` in its `card-<id>.json` file name. Neither is stored *inside* its file: `Card.id` and `BoardMeta.id` are `#[serde(skip)]`, and the two loaders (`board_store::read_board_at`, `card_store::load_card`) stamp the id from the path they read. An `"id"` key left over in an older file is ignored.

Ids are also typed and validated on the way in (`model/id.rs`): `BoardId`, `CardId`, `ListId` and `LabelId` are separate newtypes whose `Deserialize` accepts only `[A-Za-z0-9_-]{0,64}`. `storage::paths` takes only the two that name locations, so an id that identifies nothing on disk cannot build a path, and a swapped argument is a compile error.

## Why

Ids that live both in the path and in the file are one fact stored twice, and the two copies can disagree. That is not hypothetical for tct: `paths::base_dir()` adopts any `.tct` directory it finds walking up from the cwd, so the JSON may come from a checked-out repository and is fully attacker-controlled.

With the id read from the file, a record was read from one path and written back to another:

- `list_boards` read `boards/aaaa/board.json` but returned a meta whose `id` was whatever the file declared. Everything downstream (`BoardEditor::load`, `save_board`, `delete_board`) then used that id, so the board you listed was not the board you edited — or, via the hard-delete dialog, `remove_dir_all` on a directory of the file's choosing.
- `load_all_cards` read `card-a.json` and keyed it by the declared id, so saving that card wrote `card-<declared>.json` and left the original stale. Two files declaring one id silently collapsed to whichever won the directory scan.

Before the ids were validated, a declared id containing `..` escaped the data directory outright — an arbitrary file write on load (`repair_references` persists repaired cards), and an arbitrary recursive delete on "permanently delete board".

Deriving the id from the path removes the second copy, so there is nothing to disagree: read path and write path are the same path by construction, no record can shadow another, and a declared id is inert data that no code reads.

## Decisions

- **`#[serde(skip)]` over a separate stored DTO.** A `StoredCard`/`StoredBoard` mirror would make the id structurally absent, but duplicates every field and splits "add a new card field" across two types. Instead there is exactly one deserialization site per type, both documented as such, plus a `debug_assert!` in `save_card`/`save_board` that the id was stamped — an unstamped record would otherwise write itself to `card-.json`.
- **Ids stay validated even though they no longer come from files.** The charset check is still load-bearing: `board_order.json` is a list of ids, and the legacy migration reads `card_ids` out of `list-*.json` and feeds them to `load_card`. Both are file content used to build paths.
- **Lists and Labels keep their in-file ids.** They live inline in `board.json` and have no file of their own, so their ids have nowhere else to live — and `card.list_id` / `card.label_ids` reference them.
- **The empty id stays legal, but is not a path component.** `Card::new` and pre-migration files leave `list_id` empty as the "unassigned" sentinel, so the id types must admit it. It is the one admissible id that does not name a path component — `board_dir("")` is the boards directory itself — so `paths::board_dir` `debug_assert!`s against it and `delete_board` refuses it outright, that being the one irreversible caller.
- **A directory or file name that is not a valid id is not a record.** Such entries are passed over rather than treated as corrupt; a stray directory under `boards/` is not a broken board.
- **No compatibility shim.** Files written by older versions carry an `id`; it is ignored on read and dropped on the next write. Downgrading to a version that expects the field is not supported.

## Considered and rejected

- **Keep the id in the file and validate that it matches its location.** The first version of this fix. It works, but it enforces an invariant instead of removing the possibility of violating it, and it leaves the redundant field for the next reader to trust. Rejected once it was clear nothing on disk references either id.
- **Keep writing `id` as a derived field, ignore it on read.** Preserves downgrade compatibility and grep-ability at the cost of re-introducing a field that means nothing. Rejected — see "No compatibility shim".
- **Validate ids only at the point of path construction.** Equivalent in effect but repeated at every call site, and silently incomplete the moment a new call site forgets. Type-level validation at the parse boundary cannot be forgotten.

## Consequences

- A Card can be moved between boards, or renamed, by moving/renaming its file — the id follows the path and nothing references the old one.
- A board directory renamed by hand re-ids the board. Its `board_order.json` entry goes stale (the board falls back to alphabetical placement), which is cosmetic and self-correcting.
- `StorageError::IdMismatch` and its checks are gone; the failure mode they guarded cannot arise.
- Two files can no longer declare the same id, so the silent duplicate-card data loss is gone too.
