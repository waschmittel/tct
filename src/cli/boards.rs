//! `tct boards` subcommand: list/create/archive/restore/delete boards.

use super::lookup::{find_archived_board, find_board};
use super::util::{board_summary_counts, flag_value, has_flag};
use crate::model::board::BoardMeta;
use crate::model::label::LabelColor;
use crate::storage::board_store;

pub(super) fn run(args: &[String], by_id: bool) -> anyhow::Result<()> {
    if has_flag(args, "--archived") {
        let boards = board_store::list_archived_boards()?;
        if boards.is_empty() {
            println!("No archived boards.");
        } else {
            println!("Archived boards ({}):", boards.len());
            for board in &boards {
                println!("  [{}]  {}", board.id, board.name);
            }
        }
    } else if let Some(name) = flag_value(args, "--create") {
        let existing_boards = board_store::list_boards()?;
        let existing_colors: Vec<_> = existing_boards.iter().map(|b| b.accent_color).collect();
        let mut meta = BoardMeta::new(name.to_string());
        meta.accent_color = LabelColor::generate_pastel(&existing_colors);
        board_store::save_board(&meta)?;
        board_store::append_to_order(&meta.id)?;
        println!("Created board '{}'.", meta.name);
    } else if let Some(partial) = flag_value(args, "--archive") {
        let mut board = find_board(partial, by_id)?;
        let name = board.name.clone();
        board.archived = true;
        board_store::save_board(&board)?;
        board_store::remove_from_order(&board.id)?;
        println!("Archived board '{name}'.");
    } else if let Some(partial) = flag_value(args, "--restore") {
        let mut board = find_archived_board(partial, by_id)?;
        let name = board.name.clone();
        board.archived = false;
        board_store::save_board(&board)?;
        board_store::append_to_order(&board.id)?;
        println!("Restored board '{name}'.");
    } else if let Some(partial) = flag_value(args, "--delete") {
        let board = find_archived_board(partial, by_id)?;
        let name = board.name.clone();
        board_store::delete_board(&board.id)?;
        println!("Permanently deleted board '{name}'.");
    } else {
        // Default: list active boards
        let boards = board_store::list_boards()?;
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
