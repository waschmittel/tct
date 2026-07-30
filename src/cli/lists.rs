//! `tct lists <board>` subcommand: list/create/rename/archive/restore/delete lists.

use super::lookup::{find_board, find_list};
use super::util::{count_active, flag_value, flag_values2, has_flag, lists_and_cards};
use crate::outln;
use crate::board_editor::BoardEditor;
use crate::command::Command;

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
        let archived = BoardEditor::load(&board.id)?.archived_lists();
        outln!(
            "Board: {} [{}]  (archived lists: {})",
            board.name,
            board.id,
            archived.len()
        );
        for (i, list) in archived.iter().enumerate() {
            outln!(
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
        outln!("Created list '{}' on board '{}'.", name, board_name);
    } else if let Some((list_partial, new_name)) = flag_values2(args, "--rename") {
        let board = find_board(board_partial, by_id)?;
        let (lists, _) = lists_and_cards(&board);
        let list = find_list(&lists, list_partial, by_id)?.clone();
        let old_name = list.name.clone();
        let mut editor = BoardEditor::load(&board.id)?;
        editor.apply(Command::RenameList { list_id: list.id, name: new_name.to_string() })?;
        outln!("Renamed list '{old_name}' to '{new_name}'.");
    } else if let Some(list_partial) = flag_value(args, "--archive") {
        let board = find_board(board_partial, by_id)?;
        let (lists, _) = lists_and_cards(&board);
        let list = find_list(&lists, list_partial, by_id)?.clone();
        let name = list.name.clone();
        let mut editor = BoardEditor::load(&board.id)?;
        editor.apply(Command::ArchiveList { list_id: list.id })?;
        outln!("Archived list '{name}'.");
    } else if let Some(list_partial) = flag_value(args, "--restore") {
        let board = find_board(board_partial, by_id)?;
        let mut editor = BoardEditor::load(&board.id)?;
        let archived = editor.archived_lists();
        let list = find_list(&archived, list_partial, by_id)?.clone();
        let name = list.name.clone();
        editor.apply(Command::RestoreList { list_id: list.id })?;
        outln!("Restored list '{name}'.");
    } else if let Some(list_partial) = flag_value(args, "--delete") {
        let board = find_board(board_partial, by_id)?;
        let mut editor = BoardEditor::load(&board.id)?;
        let archived = editor.archived_lists();
        let list = find_list(&archived, list_partial, by_id)?.clone();
        let name = list.name.clone();
        editor.delete_archived_list(&list.id, &list.card_ids)?;
        outln!("Permanently deleted list '{name}' and its cards.");
    } else {
        // Default: list all active lists on board
        let board = find_board(board_partial, by_id)?;
        let (lists, all_cards) = lists_and_cards(&board);
        outln!("Board: {} [{}]", board.name, board.id);
        if lists.is_empty() {
            outln!("  (no lists)");
        } else {
            for (i, list) in lists.iter().enumerate() {
                let count = count_active(&list.card_ids, &all_cards);
                outln!(
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
