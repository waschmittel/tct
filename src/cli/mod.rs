//! Command-line interface — dispatch (help text lives in [`help`]).
//!
//! When `tct` is invoked with a subcommand (`boards`, `lists`, `cards`,
//! `checklist`, `labels`, `search`, `commit`), [`run`] strips the leading
//! `--by-id` flag and dispatches to the matching submodule. The
//! `--board <name>` flag (handled by [`resolve_board_flag`]) is the
//! single exception — it opens the TUI on a board rather than running a
//! headless command.

mod boards;
mod cards;
mod checklist;
mod git;
pub mod help;
mod labels;
mod lists;
mod lookup;
mod search;
mod util;

#[cfg(test)]
pub(crate) use search::card_matches_regex as card_matches_regex_pub;

use anyhow::bail;

use crate::model::id::BoardId;

/// Resolve `--board <name>` flag from args, returning the matching board's ID.
pub fn resolve_board_flag(args: &[String]) -> anyhow::Result<Option<BoardId>> {
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

/// `tct init`: create the data directory under the working directory.
fn init(rest: &[String]) -> anyhow::Result<()> {
    let git_choice = match rest {
        [] => None,
        [flag, value] if flag == "--git" => Some(git::parse_choice(Some(value))?),
        [flag] if flag == "--git" => Some(git::parse_choice(None)?),
        [extra, ..] => {
            bail!(
                "init takes only --git enable|disable (got '{extra}'); set TCT_DATA_DIR to pick the folder"
            )
        }
    };
    let existed = crate::storage::board_store::init_data_dir()?;
    let verb = if existed {
        "Reinitialized existing"
    } else {
        "Initialized empty"
    };
    crate::outln!(
        "{verb} tct data directory in {}",
        crate::storage::paths::base_dir().display()
    );
    git::init_step(git_choice)
}

/// `tct --git [enable|disable]` (the data directory must exist).
pub fn git_flag(rest: &[String]) -> anyhow::Result<()> {
    crate::storage::board_store::open_data_dir()?;
    git::flag(rest)
}

pub fn run(args: &[String]) -> anyhow::Result<()> {
    if args[0] == "init" {
        return init(&args[1..]);
    }
    crate::storage::board_store::open_data_dir()?;
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
        "commit" => git::commit(rest),
        other => bail!("Unknown command '{other}'. Run 'tct --help' for usage."),
    }
}
