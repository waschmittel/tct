//! `tct boards` subcommand: list/create/archive/restore/delete boards.

use super::lookup::{find_archived_board, find_board};
use super::util::{board_summary_counts, flag_value, has_flag};
use crate::board_directory;

pub(super) fn run(args: &[String], by_id: bool) -> anyhow::Result<()> {
    if has_flag(args, "--archived") {
        let boards = board_directory::list_archived()?;
        if boards.is_empty() {
            println!("No archived boards.");
        } else {
            println!("Archived boards ({}):", boards.len());
            for board in &boards {
                println!("  [{}]  {}", board.id, board.name);
            }
        }
    } else if let Some(name) = flag_value(args, "--create") {
        let meta = board_directory::create(name.to_string())?;
        println!("Created board '{}'.", meta.name);
    } else if let Some(partial) = flag_value(args, "--archive") {
        let board = find_board(partial, by_id)?;
        board_directory::archive(&board.id)?;
        println!("Archived board '{}'.", board.name);
    } else if let Some(partial) = flag_value(args, "--restore") {
        let board = find_archived_board(partial, by_id)?;
        board_directory::restore(&board.id)?;
        println!("Restored board '{}'.", board.name);
    } else if let Some(partial) = flag_value(args, "--delete") {
        let board = find_archived_board(partial, by_id)?;
        board_directory::delete(&board.id)?;
        println!("Permanently deleted board '{}'.", board.name);
    } else {
        // Default: list active boards
        let boards = board_directory::list()?;
        if boards.is_empty() {
            println!("No active boards.");
        } else {
            println!("Active boards ({}):", boards.len());
            for board in &boards {
                let (lists, total_cards) = board_summary_counts(&board.id);
                println!(
                    "  [{}]  {:<30} {} lists, {} active cards",
                    board.id, board.name, lists, total_cards
                );
            }
        }
    }
    Ok(())
}
