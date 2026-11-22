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
- A *discovered* `.tct` must be a real directory (symlinks are skipped). One
  owned by another user (Unix) is refused with an error naming it and the
  `TCT_DATA_DIR` override, as git refuses a repository owned by someone else.
  `TCT_DATA_DIR` and `~/.tct` remain the user's explicit choice.

## Rejected

- `O_NOFOLLOW` on a fixed temp name: still racy between two tct processes and
  leaves stale files.
- Skipping a foreign `.tct` and walking on: silently lands in a different data
  directory (usually `~/.tct`), and the user edits the wrong boards.
- Mode checks (0700, or not world-writable): git checkouts and umask 002
  setups would break the former; the latter only catches a 0777 `.tct` you own
  yourself, which a planted one never is — the owner check already covers it.
