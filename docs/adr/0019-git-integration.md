# Git integration: explicit commits of the data directory, nothing else

## Decision

tct can commit its data directory to the git repository it lives in (`src/git_sync/`).

- **Triggers are explicit.** `tct commit` (no arguments) and `C` in the TUI board view commit now. When switched on with `tct --git enable`, quitting the TUI commits too. The CLI's other commands never commit, and nothing commits in the background — one session yields one commit, and a crash loses nothing (the files stay in the working tree for the next commit).
- **Only the data directory.** Every git call is `git -C <data dir> … -- .`: `add -A -- .`, then a partial commit (`commit -- .`) of that same pathspec. Anything else staged in the repository stays staged and out of the commit. A data directory that *is* the repository root, or is git-ignored, is refused — `.` would be the whole repository.
- **The opt-in is local git config** (`tct.autocommit`), not a file in `.tct`. The data directory may come from a checked-out repository (AGENTS.md, "Untrusted data dir"); a committed switch would let a clone turn auto-committing on. It only governs commit-on-quit; explicit commits always work inside a repository.
- **`tct init`** enables it when the new data directory is inside a work tree that does not ignore it, `--git enable|disable` overrides, a re-init keeps an existing setting, and it always prints the outcome and why.
- **Shelling out to `git`**, not libgit2/gix: no dependency, and the user's identity and config apply. stdin is null, `GIT_TERMINAL_PROMPT=0`, inherited `GIT_DIR`/`GIT_WORK_TREE`/`GIT_INDEX_FILE` are removed. Commits use `--no-verify --no-gpg-sign` so an automatic commit can never block on a hook or a pinentry. Merge, rebase, cherry-pick and revert in progress refuse the commit.
- **Messages name titles, not ids**, and every subject starts with `TCT: ` (`message::SUBJECT_PREFIX`) so tct commits stand out in `git log`. The changed paths of the staged diff are mapped to boards and cards, old versions are read from `HEAD` and new ones from the index through the stores' own parse functions (`board_store::parse_board`, `card_store::parse_card` — no third parse recipe). One board and one change: the subject says it all (`Board "Sprint": card "Fix login" moved to "Done"`). Several changes: `Board "Sprint": 3 cards updated` plus one line per change. Several boards: `Boards "A", "B" updated` plus a section per board. Detail lines are capped at 20; titles are flattened to one line and shortened (they are untrusted).

## Rejected

- **Committing on every write or on a timer** — hundreds of commits per session, races with the periodic reload and with the user's own git commands.
- **A `-m` override for `tct commit`** — one message generator, so explicit and on-quit commits are indistinguishable.
- **Committing after each CLI command** — scripts do many commands; `tct commit` once is the batch.
- **A nested repository inside `.tct`** — cleaner history, but not "the repository the project is in".
- **A committed marker file** — see above.
- **libgit2/gix** — heavy dependency, ignores the user's git configuration.

## Limits

Changes by a crash or kill are committed only by the next commit. Concurrent tct instances share the working tree, so whoever commits first commits all. A repository whose `git` needs interaction (credentials are not involved — commits are local) is not supported.
