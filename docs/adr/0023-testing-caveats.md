# Testing: isolation from real data, and other non-obvious caveats

## Decision

**Unit tests never touch a real data directory.** `paths::base_dir()` walks up from the cwd and adopts any `.tct` it finds (ADR-0007), so a test that creates boards without setting `TCT_DATA_DIR` silently writes into the *project's own* `.tct` (or `~/.tct`). That happened: two tests built a board outside `with_temp_dir` and left `Test` boards behind on every run, which then got committed. Therefore:

- Every unit test that reads or writes storage runs inside `test_support::with_temp_dir(|| …)` (fresh `TempDir` as `TCT_DATA_DIR`). Fixture helpers (`editor_with_cards`, `single_list_fixture`, …) do not set it up themselves; their callers must.
- In `cfg(test)` builds `storage::paths::resolve` calls `forbid_real_data_dir` and **panics** when the resolved directory is not under the system temp dir, so a forgotten wrapper fails loudly instead of writing. A test that only wants to inspect discovery (no I/O) calls `resolve_unchecked()`.
- Integration tests (`tests/`) run the binary and set `TCT_DATA_DIR` to a `TempDir` (or `<dir>/.tct` inside one) per test; the guard does not apply there, so keep that rule.
- Agents and humans likewise run no manual experiments against the project's `.tct`: use `TCT_DATA_DIR=$(mktemp -d)`.

## Other caveats worth knowing

- **Single-threaded:** `cargo test -- --test-threads=1`. `TCT_DATA_DIR` and the cwd are process-global; tests that change them (`with_temp_dir`, discovery tests using `set_current_dir`) race otherwise. A panic in a test that changed the cwd leaves it in a deleted temp dir and makes *other* tests fail with `NotFound` — read the first failure, not the last.
- **Golden screens** (`ui/snapshot_tests/`, insta): a golden is named after the test's module path, so moving or renaming a test renames its `.snap` (ADR-0020). Fixtures must not contain due dates or history timestamps (rendered relative to now → churn).
- **Example PDFs** (`docs/export-examples/`) are compared byte for byte by `example_pdfs_are_up_to_date`. Anything that can change PDF output needs `TZ=UTC cargo test write_example_pdfs -- --ignored`; `.cargo/config.toml` pins `TZ=UTC` and the render uses a fixed `EXAMPLE_TIME`, so output only changes with content.
- **Git tests** (`git_sync`, `tests/git.rs`) need `git` installed and assert that nothing outside the data dir is ever committed (ADR-0019).
- **First build** with a fresh checkout needs network once (rustup installs the pinned toolchain, cargo fetches crates); later runs work offline.
- **VHS tapes** are platform/font dependent and are re-recorded only before a release (`docs/vhs/check.sh`), never per change.

## Rejected

- Trusting reviewers/habit to remember `with_temp_dir`: it was forgotten, silently, for a long time.
- Having every fixture helper call `with_temp_dir` itself: nesting it would reset the env mid-test; the wrapper stays at the test boundary.
- Failing on *any* use of the home or discovered dir in all builds: production must keep discovering `.tct`; only `cfg(test)` is guarded.
