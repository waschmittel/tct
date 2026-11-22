# ADR-0026: Symlink-safe writes and trusted data-dir discovery

Status: accepted

## Context

ADR-0007 and the "untrusted data dir" invariants made *reads* safe: ids are
validated and `read_unlinked` refuses symlinks. Writes were not: `atomic_write`
wrote to `path.with_extension("tmp")` with `fs::write`, which follows symlinks.
A repository shipping `.tct/.version.tmp -> ~/victim` made a plain `tct boards`
overwrite `~/victim` (no `.version` means the stamp is written on open); a
planted `card-<id>.tmp` did the same on the first edit of that card. Separately,
discovery adopted any `.tct` found walking up from the cwd, including one
planted by another user in a shared directory such as `/tmp`.

## Decision

- `atomic_write` creates a random-named sibling with `tempfile::NamedTempFile`
  (`O_EXCL`, mode 0600), syncs it and renames it over the target. `O_EXCL` never
  follows a symlink, and there is no predictable name to plant.
- Directories tct creates are 0700 (`create_private_dir_all`).
- A *discovered* `.tct` must be a real directory owned by the current user and
  not world-writable (Unix). Others are skipped like symlinks. `TCT_DATA_DIR`
  and `~/.tct` remain the user's explicit choice.

## Rejected

- `O_NOFOLLOW` on a fixed temp name: still racy between two tct processes and
  leaves stale files.
- Requiring 0700 on `.tct`: git checkouts and umask 002 setups would break;
  only world-writable is refused.
