//! Command-line interface — dispatch and help text.
//!
//! When `tct` is invoked with a subcommand (`boards`, `lists`, `cards`,
//! `checklist`, `labels`, `search`), [`run`] strips the leading
//! `--by-id` flag and dispatches to the matching submodule. The
//! `--board <name>` flag (handled by [`resolve_board_flag`]) is the
//! single exception — it opens the TUI on a board rather than running a
//! headless command.

mod boards;
mod cards;
mod checklist;
mod labels;
mod lists;
mod lookup;
mod search;
mod util;

#[cfg(test)]
pub(crate) use search::card_matches_regex as card_matches_regex_pub;

use anyhow::bail;

const HELP: &str = "\
tct - Terminal Card Tracker

A keyboard-driven TUI Kanban board. Run without arguments to open the TUI.

USAGE:
    tct                              Open the TUI board selector
    tct --board <name>               Open TUI directly on a board (partial, case-insensitive)
    tct --help, -h                   Show this help message
    tct <COMMAND> [ARGS] [FLAGS]     Run a CLI command (no TUI)

FLAGS (apply to any command):
    --by-id                          Match all identifier arguments by exact ID instead of name

COMMANDS:
  boards                              List active boards
  boards --archived                   List archived boards
  boards --create <name>              Create a new board
  boards --archive <name>             Archive a board
  boards --restore <name>             Restore an archived board
  boards --delete <name>              Permanently delete an archived board

  lists <board>                       List all active lists on a board
  lists <board> --archived            List archived lists
  lists <board> --create <name>       Create a list
  lists <board> --rename <list> <name>  Rename a list
  lists <board> --archive <list>      Archive a list
  lists <board> --restore <list>      Restore an archived list
  lists <board> --delete <list>       Permanently delete an archived list and its cards

  cards <board>                       List active cards grouped by list
  cards <board> --archived            List archived cards
  cards <board> --list <list>         List active cards in a specific list
  cards <board> --show <card>         Show full card detail
  cards <board> --create <list> <title>  Create a card in a list
  cards <board> --edit <card>         Edit card fields
    --title <text>                      New title
    --description <text>                New description
    --due <YYYY-MM-DD|none>             Set or clear due date
  cards <board> --archive <card>      Archive a card
  cards <board> --restore <card>      Restore an archived card to the first list
  cards <board> --delete <card>       Permanently delete an archived card

  checklist <board> <card>            Show checklist
  checklist <board> <card> --add <text>     Add a checklist item
  checklist <board> <card> --toggle <n>     Toggle item n (1-based index)
  checklist <board> <card> --delete <n>     Delete item n (1-based index)

  labels <board>                      List all labels
  labels <board> --create <name>      Create a label
  labels <board> --delete <label>     Delete a label (removes from all cards)
  labels <board> --assign <card> <label>   Assign a label to a card
  labels <board> --remove <card> <label>   Remove a label from a card

  search <query>                      Search cards across all boards (case-insensitive substring)
  search <query> --board <name>       Limit search to boards matching name (repeatable)
  search <query> --list <name>        Limit search to lists matching name
  search <query> --regex              Treat query as a regular expression
  search <query> --archived           Include archived cards in results

IDs are shown in listings as [xxxxxxxx]. Pass --by-id to match by ID instead of name.
Multiple name matches or a missing ID result in an error.

STORAGE:
    By default data is stored in ~/.tct/. If a .tct/ directory exists in the current
    working directory or any of its parents, that directory is used instead.
    Override with the TCT_DATA_DIR environment variable.
";

pub fn print_help() {
    print!("{HELP}");
}

/// Resolve `--board <name>` flag from args, returning the matching board's ID.
pub fn resolve_board_flag(args: &[String]) -> anyhow::Result<Option<String>> {
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--board" {
            if i + 1 < args.len() {
                let partial = &args[i + 1];
                let board = lookup::find_board(partial, false)?;
                return Ok(Some(board.id));
            } else {
                bail!("--board requires a board name argument");
            }
        }
        i += 1;
    }
    Ok(None)
}

pub fn run(args: &[String]) -> anyhow::Result<()> {
    crate::storage::board_store::ensure_base_dirs()?;
    let by_id = args.iter().any(|a| a == "--by-id");
    let args: Vec<String> = args.iter().filter(|a| *a != "--by-id").cloned().collect();
    let sub = args[0].as_str();
    let rest = &args[1..];
    match sub {
        "boards" => boards::run(rest, by_id),
        "lists" => lists::run(rest, by_id),
        "cards" => cards::run(rest, by_id),
        "checklist" => checklist::run(rest, by_id),
        "labels" => labels::run(rest, by_id),
        "search" => search::run(rest),
        other => bail!("Unknown command '{other}'. Run 'tct --help' for usage."),
    }
}
