# A versioned data directory, and no on-load migrations

The data directory carries its format version in `.tct/.version`, checked once at startup by `storage::version::ensure_compatible` (reached through `board_store::open_data_dir`, which every entry point already called). A directory whose version is newer than the binary is refused; so is one that is older. Nothing else in the codebase knows about format history: the loaders parse exactly one shape.

This supersedes the "auto-migration on load" decision in [ADR-0006](0006-card-owned-list-membership.md).

## Why

Migrations had leaked into the read path and into the domain:

- `card_store::load_card` parsed every Card into a `serde_json::Value` first so it could rewrite two legacy shapes (`checklists` → `checklist`, inline `labels` → `label_ids`), then re-serialised and saved. Every read of every card paid for a format two versions dead, and the loader's straight-line "parse a Card" was buried in `Value` munging.
- `board_store::load_board` called `migrate_if_needed`, which scanned the board directory for `list-*.json` on every single load, so the check ran forever for a conversion that happens once.
- `BoardMeta.list_order` sat on the domain type purely so the migration could read it, describing a layout that no longer exists.
- `list_store` survived as `#[cfg(test)]` "to build legacy fixtures" — and a dozen test modules used it, which meant most fixtures seeded the *legacy* format and depended on the migration to produce the modern one. The tests were exercising the migration by accident rather than the behaviour they named.

A version file replaces all of it with one check in one place. It also closes a hazard introduced by ADR-0007: formats now change by *removing* fields, so an older binary opening a newer directory used to fail deep in serde with "missing field `id`", or load a partial record and write it back. It now says what is actually wrong.

## Decisions

- **Absent `.version` means the current version, and gets stamped.** Every existing installation has no such file; refusing them would be absurd. The formats that predate versioning are unreadable anyway, so "absent" cannot usefully mean anything else.
- **Older versions are refused, not migrated.** The `VersionTooOld` arm is where a future migration would go, and it is empty. When one is needed it will live there — explicit, one-shot, at startup — not in a loader.
- **The gate lives inside `open_data_dir`, not beside it.** Both entry points already called `ensure_base_dirs` before touching storage; folding the check into it (and renaming) means a future entry point cannot forget the gate.
- **`.version` is untrusted like everything else** under a discovered `.tct`: read through `read_unlinked` (so not a symlink) and parsed as a single integer, with a distinct error when it is not.
- **No compatibility shims elsewhere.** `serde(default)` on fields stays, because it makes *forward* compatibility cheap within a version; what is gone is code that recognises and rewrites a *previous* version's shape.

## Considered and rejected

- **Keep the legacy `list-*.json` migration, just move it behind the version gate.** Would preserve the conversion for anyone still on that format, but it is the format from before ADR-0006 and the code to support it is larger than the code that reads the current one. Rejected as dead weight; such a user can convert with the version that wrote it.
- **Detect legacy layouts structurally instead of with a version file** (e.g. "are there `list-*.json` files?"). That is a migration check by another name, and it has to run on every load to be reliable. Rejected.
- **Timestamped or semver format versions.** A monotonic integer is enough to answer the only two questions asked: can this binary read it, and can it write it.

## Consequences

- `storage/migrate.rs` and `storage/list_store.rs` are deleted, with `paths::list_path` and `StorageError::ListNotFound`.
- `load_card` is a direct `serde_json::from_str` into a `Card`; no `Value`, no re-save on read.
- `CardList` loses its serde derives and its `archived` field — it is a derived in-memory view and was only ever serialised into legacy list files.
- Test fixtures seed the current format through `test_support::add_list` / `put_cards`. They no longer round-trip through a migration to reach the state they claim to set up.
- A data directory older than the current format fails loudly at startup instead of loading half-converted.
