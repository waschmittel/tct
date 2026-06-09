//! `tct lists <board>` subcommand: list/create/rename/archive/restore/delete lists.

use super::lookup::{find_board, find_list};
use super::util::{count_active, flag_value, flag_values2, has_flag, load_all_cards};
use crate::board_editor::BoardEditor;
use crate::command::Command;
use crate::storage::{card_store, list_store};

pub(super) fn run(args: &[String], by_id: bool) -> anyhow::Result<()> {
    let board_partial = args
        .first()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Usage: tct lists <board> [--create|--rename|--archive|--restore|--delete|--archived ...]"
            )
        })?
        .as_str();

    if has_flag(args, "--archived") {
        let board = find_board(board_partial, by_id)?;
        let archived = list_store::list_archived_lists(&board.id);
        println!(
            "Board: {} [{}]  (archived lists: {})",
            board.name,
            board.id,
            archived.len()
        );
        for (i, list) in archived.iter().enumerate() {
            println!(
                "  {}. [{}]  {}  ({} cards)",
                i + 1,
                list.id,
                list.name,
                list.card_ids.len()
            );
        }
    } else if let Some(name) = flag_value(args, "--create") {
        let board = find_board(board_partial, by_id)?;
        let board_name = board.name.clone();
        let mut editor = BoardEditor::load(&board.id)?;
        editor.apply(Command::AddList { name: name.to_string() })?;
        println!("Created list '{}' on board '{}'.", name, board_name);
    } else if let Some((list_partial, new_name)) = flag_values2(args, "--rename") {
        let board = find_board(board_partial, by_id)?;
        let lists = list_store::load_all_lists(&board.id, &board.list_order)?;
        let list = find_list(&lists, list_partial, by_id)?.clone();
        let old_name = list.name.clone();
        let mut editor = BoardEditor::load(&board.id)?;
        editor.apply(Command::RenameList { list_id: list.id, name: new_name.to_string() })?;
        println!("Renamed list '{old_name}' to '{new_name}'.");
    } else if let Some(list_partial) = flag_value(args, "--archive") {
        let board = find_board(board_partial, by_id)?;
        let lists = list_store::load_all_lists(&board.id, &board.list_order)?;
        let list = find_list(&lists, list_partial, by_id)?.clone();
        let name = list.name.clone();
        let mut editor = BoardEditor::load(&board.id)?;
        editor.apply(Command::ArchiveList { list_id: list.id })?;
        println!("Archived list '{name}'.");
    } else if let Some(list_partial) = flag_value(args, "--restore") {
        let board = find_board(board_partial, by_id)?;
        let archived = list_store::list_archived_lists(&board.id);
        let list = find_list(&archived, list_partial, by_id)?.clone();
        let name = list.name.clone();
        let mut editor = BoardEditor::load(&board.id)?;
        editor.apply(Command::RestoreList { list_id: list.id })?;
        println!("Restored list '{name}'.");
    } else if let Some(list_partial) = flag_value(args, "--delete") {
        let board = find_board(board_partial, by_id)?;
        let archived = list_store::list_archived_lists(&board.id);
        let list = find_list(&archived, list_partial, by_id)?.clone();
        let name = list.name.clone();
        for card_id in &list.card_ids {
            let _ = card_store::delete_card(&board.id, card_id);
        }
        list_store::delete_list_file(&board.id, &list.id)?;
        println!("Permanently deleted list '{name}' and its cards.");
    } else {
        // Default: list all active lists on board
        let board = find_board(board_partial, by_id)?;
        let lists = list_store::load_all_lists(&board.id, &board.list_order)?;
        let all_cards = load_all_cards(&board.id, &lists);
        println!("Board: {} [{}]", board.name, board.id);
        if lists.is_empty() {
            println!("  (no lists)");
        } else {
            for (i, list) in lists.iter().enumerate() {
                let count = count_active(&list.card_ids, &all_cards);
                println!(
                    "  {}. [{}]  {}  ({} active cards)",
                    i + 1,
                    list.id,
                    list.name,
                    count
                );
            }
        }
    }
    Ok(())
}
