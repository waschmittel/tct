//! `tct boards` subcommand: list/create/archive/restore/delete boards.

use super::lookup::{find_archived_board, find_board};
use super::util::{board_summary_counts, flag_value, has_flag};
use crate::outln;
use crate::board_directory;

pub(super) fn run(args: &[String], by_id: bool) -> anyhow::Result<()> {
    if has_flag(args, "--archived") {
        let boards = board_directory::list_archived()?;
        if boards.is_empty() {
            outln!("No archived boards.");
        } else {
            outln!("Archived boards ({}):", boards.len());
            for board in &boards {
                outln!("  [{}]  {}", board.id, board.name);
            }
        }
    } else if let Some(name) = flag_value(args, "--create") {
        let meta = board_directory::create(name.to_string())?;
        outln!("Created board '{}'.", meta.name);
    } else if let Some(partial) = flag_value(args, "--archive") {
        let board = find_board(partial, by_id)?;
        board_directory::archive(&board.id)?;
        outln!("Archived board '{}'.", board.name);
    } else if let Some(partial) = flag_value(args, "--restore") {
        let board = find_archived_board(partial, by_id)?;
        board_directory::restore(&board.id)?;
        outln!("Restored board '{}'.", board.name);
    } else if let Some(partial) = flag_value(args, "--delete") {
        let board = find_archived_board(partial, by_id)?;
        board_directory::delete(&board.id)?;
        outln!("Permanently deleted board '{}'.", board.name);
    } else {
        // Default: list active boards
        let boards = board_directory::list()?;
        if boards.is_empty() {
            outln!("No active boards.");
        } else {
            outln!("Active boards ({}):", boards.len());
            for board in &boards {
                let (lists, total_cards) = board_summary_counts(&board.id);
                outln!(
                    "  [{}]  {:<30} {} lists, {} active cards",
                    board.id, board.name, lists, total_cards
                );
            }
        }
    }
    Ok(())
}
