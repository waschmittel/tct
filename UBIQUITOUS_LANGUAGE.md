# Ubiquitous Language

> Seeded from codebase scan (no prior conversation). Domain: **tct** — a terminal-based kanban/task tool inspired by Trello, with modal TUI, JSON-on-disk storage, and a CLI.

## Domain objects

| Term          | Definition                                                                                       | Aliases to avoid                |
| ------------- | ------------------------------------------------------------------------------------------------ | ------------------------------- |
| **Board**     | Top-level workspace owning an ordered set of **Lists**, a label palette, and an accent color     | Project, workspace              |
| **List**      | An ordered column on a **Board** holding **Cards** (typed as `CardList` to avoid stdlib clash)   | Column, lane, stack             |
| **Card**      | A unit of work belonging to one **List**; carries title, description, checklist, labels, history | Task, ticket, item, note        |
| **Checklist** | The ordered set of **Checklist Items** on a **Card**                                             | Subtasks, todo list             |
| **Checklist Item** | A single checkable line within a **Card**'s checklist                                       | Subtask, todo, step             |
| **Label**     | Named, colored tag attached to **Cards**; defined per-**Board** in a label palette               | Tag, category                   |
| **Label Color** | Pastel color value for a **Label** — either a named variant (Red…Cyan) or `Custom { r, g, b }` | Hue, swatch                     |
| **History Entry** | A timestamped action recorded on a **Card** (capped at `HISTORY_LIMIT = 50`)                | Audit log, changelog            |
| **Accent Color** | Per-**Board** UI highlight color (`BoardMeta.accent_color`); replaces hardcoded Cyan         | Theme color, primary color      |

## State & lifecycle

| Term            | Definition                                                                                    | Aliases to avoid                |
| --------------- | --------------------------------------------------------------------------------------------- | ------------------------------- |
| **Archived**    | Soft-deleted state on **Board**/**List**/**Card** (`archived: bool`); hidden but recoverable  | Deleted, trashed, removed       |
| **Loaded Board**| In-memory aggregate of one **Board**'s meta, **Lists**, **Cards** owned by a **Board Editor** | Active board, current board     |
| **Board Editor**| Aggregate root owning a **Loaded Board** + **Selection State**; exposes domain verbs (`add_card_to_list`, `archive_card`, `move_card`, …) and atomic stage-and-commit persistence | Service, manager, repository |
| **Board Directory** | Module owning the collection of **Boards**: create, archive/restore, rename, display order, listing (`src/board_directory.rs`). Everything that spans multiple boards or runs before a board is loaded | Board manager, board service |
| **Selection State** | Navigation invariants for a **Loaded Board** — selected list/card indices, scroll offsets, search query, label filter; mutated only via verbs that maintain clamp/shift invariants | Cursor, focus               |
| **Visible**     | A **Card** shown given its **Archived** flag and the active search (`LoadedBoard::visible_cards` is the single source of truth; navigation, clamping, and rendering all consume it) | Filtered, shown, matching |
| **Touch**       | Bump a **Card**'s `updated_at` timestamp without other change                                 | Update, mark dirty              |
| **Log**         | Append a **History Entry** to a **Card** and touch it                                         | Record, track                   |

## UI modes & input

| Term            | Definition                                                                                    | Aliases to avoid                |
| --------------- | --------------------------------------------------------------------------------------------- | ------------------------------- |
| **App Mode**    | Top-level modal state (`BoardSelector`, `Normal`, `CardDetail`, `Insert`, `Command`, `Dialog`, `Help`) | State, screen, view      |
| **Insert Handler** | A struct implementing `InsertHandler` for one editable target (new card title, description, due date, …); the active one is `Box<dyn InsertHandler>` on `App` (ADR-0003) | Insert target, edit target, field |
| **Dialog** | A struct implementing the `Dialog` trait for one modal kind (confirmations, label picker, archived lists, card history, …); active one is `Box<dyn Dialog>` (ADR-0003) | Dialog kind, popup, prompt, modal |
| **Board Selector** | The screen listing all **Boards** for opening, renaming, archiving                         | Board picker, home screen       |
| **Card Detail** | Focused view of one **Card** with its tabs (description, checklist, labels, history)          | Card view, detail panel         |
| **Grab mode**   | Transient state where the selected **Card** moves with cursor keys                            | Drag, pickup, move mode         |

## Storage

| Term            | Definition                                                                                    | Aliases to avoid                |
| --------------- | --------------------------------------------------------------------------------------------- | ------------------------------- |
| **Data dir**    | Root directory for all **Board** files; defaults to `~/.tct/boards/`, overridable via `TCT_DATA_DIR` | Storage path, root         |
| **Atomic write**| Write to `.tmp` then rename; used for every persisted file                                    | Safe write, transactional save  |
| **Short ID**    | Compact identifier (`ShortId`) used for **Boards**, **Lists**, **Cards**, **Labels**          | UUID, key, handle               |
| **Periodic reload** | Filesystem re-read every `reload_interval` (15s default) on `App::on_tick`, skipped during edit/dialog/grab | Refresh, sync, poll |

## Relationships

- A **Board** owns 0..N **Lists**, ordered by the inline `BoardMeta.lists` array (`list_order` is a legacy migration-only field)
- A **List** owns 0..N **Cards**, ordered by each **Card**'s `position` (fractional rank); `CardList.card_ids` is the derived in-memory order
- A **Card** belongs to exactly one **List** at a time, via its own `list_id`
- A **Card** has 0..N **Labels**, each referencing a **Label** defined on its parent **Board**
- A **Card** has 0..N **Checklist Items** and 0..N **History Entries** (capped at 50)
- A **Board** has exactly one **Accent Color**; **Labels** each have one **Label Color**
- **Archived** is independent at each level — a non-archived **Card** in an archived **List** is still hidden

## Example dialogue

> **Dev:** "When the user presses `a` on the board view, do we delete the **Card** or archive it?"

> **Domain expert:** "Archive. Nothing is ever deleted — **Cards**, **Lists**, and **Boards** all have an `archived` flag. The user can reopen the **Archived Cards** dialog and restore."

> **Dev:** "So if I archive a **List**, the **Cards** inside it stay non-archived?"

> **Domain expert:** "Right. **Archived** is independent at each level. The **List** hides, but its **Cards** keep their own flag — restore the **List** and they reappear unchanged."

> **Dev:** "And if I edit a **Card**'s title, does that add a **History Entry**?"

> **Domain expert:** "Yes — `card.log(...)` appends a **History Entry** and touches `updated_at` in one step. We cap at 50 entries, FIFO."

> **Dev:** "What's the difference between a **Label**'s color and the **Board**'s accent color?"

> **Domain expert:** "Both are `LabelColor` values, but they serve different roles. A **Label Color** tags a **Card**. The **Accent Color** is the **Board**'s UI highlight — replaces the old hardcoded Cyan. Same type, different scope."

## Flagged ambiguities

- **"List"** is overloaded — the domain object collides with stdlib/`Vec`. Codebase resolves this by naming the struct `CardList`. In docs and chat, prefer **List** for the domain concept; reserve `Vec`/`Vec<T>` for the data structure.
- **"Archived"** vs **"Deleted"** — there is no true delete in the domain; every "delete" action is an archive. Avoid the word **delete** in user-facing copy unless referring to **Labels**, which *are* hard-deleted (see `ConfirmDeleteLabel`).
- **"Color"** alone is ambiguous — clarify as **Label Color** (per-label) or **Accent Color** (per-board). Both share the `LabelColor` type but serve different roles.
- **"Edit"** vs **"Insert"** — the `Insert` **App Mode** covers both new-entity creation *and* editing existing fields (distinct **Insert Handler** structs, e.g. a new-card-title handler vs an edit-title handler — ADR-0003). Prefer **Insert mode** for the mode itself; say "create" or "edit" when describing the user-visible action.
- **"List"** in `CardDetailTab` and in `BoardMeta.lists` always means the domain **List**, never a generic collection. Method names like `clamp_selection` operate on the list-of-lists.
