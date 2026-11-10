# Directory modules for large files; tests beside, not inside

## Decision

Files that outgrew ~1000 lines became directory modules, one file per concern. The change is a pure move: no logic changed, callers' paths are unchanged (`pub use`/`pub(crate) use` in `mod.rs`), and every test kept its name.

- **`mod.rs` keeps the type and the public surface.** Constants, shared types and the struct definition stay there; `impl` blocks are split by topic into sibling files (`board_editor/{apply,selection,moves,…}.rs`, `export/pdf/{flow,notes,table,…}.rs`). Siblings reach each other through `pub(super)` items; a child module sees its parent's private items, so fields stay private.
- **Tests are a module of their own** (`tests.rs`, or `tests/` with one file per topic and shared helpers in `tests/mod.rs`) — never an inline `mod tests { … }` in a large file. The remaining inline ones are small.
- **Golden screens** moved to `ui/snapshot_tests/` grouped by screen. insta names a golden after the test's *module path*, so each golden was renamed with its test (`…snapshot_tests__<group>__<test>.snap`) and lives in `ui/snapshot_tests/snapshots/`; contents are byte-identical.
- **Integration tests** (`tests/cli_examples/main.rs` + one file per subcommand) stay a single test binary.

## Safety net used for the move

Before and after: identical sorted list of test names (951), all tests green, zero warnings (`cargo clippy --all-targets`), `cargo fmt --check`, golden bodies unchanged with `INSTA_UPDATE=no`, and the example PDFs regenerated with `TZ=UTC cargo test write_example_pdfs -- --ignored` byte-identical.

## Rejected

- **Splitting `BoardEditor::apply` (a ~480-line `match`) into per-domain handlers** — that changes logic, not layout; worth doing separately with its own review.
- **One crate per layer** — the module boundaries here are not stable enough yet to pay for the build and visibility overhead.
