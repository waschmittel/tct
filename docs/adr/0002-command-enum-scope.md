# Command enum is the only chokepoint for domain mutations

`src/command.rs` defines a `Command` enum (variants like `AddCard`, `ArchiveCard`, `MoveCard`, `RenameList`, `DefineLabel`, …) covering every **Board** / **List** / **Card** / **Label** mutation. Mutations reach the **Board Editor** only via `BoardEditor::apply(cmd) -> Result<(), BoardEditorError>`. Selection state changes do **not** go through `Command`.

## Why

Bundling mutation + History Entry + persistence in one `apply` step makes it impossible to mutate a Card without producing the History Entry. The type system enforces what discipline did before. Commands are also inspectable test fixtures: tests construct `Command` values and apply them in-process, instead of driving the TUI or shelling out to the CLI.

## Considered and rejected

- **All mutations including selection through `Command`.** Rejected — selection moves don't produce History Entries and don't touch disk. Forcing them through the same pipeline buys uniformity at the cost of an awkward branch inside `apply`. Selection stays as direct methods on Board Editor.
- **Methods on Board Editor with a private `CardMutation` trait, no public enum.** Rejected — loses the inspectable command stream that makes in-process testing cheap and undo future-feasible.
- **Build an undo stack now.** Rejected — `Command` is invertible-by-design (each variant carries enough info to construct its inverse), but no undo stack ships until a user asks. YAGNI.

## Consequences

- Future Command variants are added once; both CLI and TUI front-ends get them for free (see ADR-0001).
- An undo stack can be added later by capturing `Vec<Command>` history and a small `invert(&self) -> Command` impl; the call-site shape doesn't change.
- Selection verbs (`select_next_card`, `set_search`, `set_label_filter`, …) are direct methods on Board Editor and are not Commands.
