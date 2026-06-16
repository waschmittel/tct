//! `tct cards <board>` subcommand: list/show/create/edit/archive/restore/delete cards.

use anyhow::{bail, Context};
use chrono::NaiveDate;

use super::lookup::{find_archived_card, find_board, find_card_in_lists, find_list};
use super::util::{
    flag_value, flag_values2, has_flag, lists_and_cards, print_card_detail, print_card_line,
};
use crate::board_editor::BoardEditor;
use crate::command::Command;
use crate::storage::card_store;

pub(super) fn run(args: &[String], by_id: bool) -> anyhow::Result<()> {
    let board_partial = args
        .first()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Usage: tct cards <board> [--show|--create|--edit|--archive|--restore|--delete|--list|--archived ...]"
            )
        })?
        .as_str();

    if has_flag(args, "--archived") {
        let board = find_board(board_partial, by_id)?;
        let archived = card_store::list_archived_cards(&board.id);
        println!(
            "Board: {} [{}]  (archived cards: {})",
            board.name,
            board.id,
            archived.len()
        );
        for (i, card) in archived.iter().enumerate() {
            let date = card.updated_at.format("%Y-%m-%d");
            println!(
                "  {}. [{}]  {}  (archived: {})",
                i + 1,
                card.id,
                card.title,
                date
            );
        }
    } else if let Some(card_partial) = flag_value(args, "--show") {
        let board = find_board(board_partial, by_id)?;
        let (lists, all_cards) = lists_and_cards(&board);
        let (list, card) = find_card_in_lists(&lists, &all_cards, card_partial, false, by_id)?;
        print_card_detail(&card, &board, &list);
    } else if let Some(list_partial) = flag_value(args, "--list") {
        let board = find_board(board_partial, by_id)?;
        let (lists, all_cards) = lists_and_cards(&board);
        let list = find_list(&lists, list_partial, by_id)?.clone();
        let active: Vec<_> = list
            .card_ids
            .iter()
            .filter_map(|id| all_cards.get(id))
            .filter(|c| !c.archived)
            .collect();
        println!(
            "Board: {} [{}]  List: {} [{}]  ({} active cards)",
            board.name,
            board.id,
            list.name,
            list.id,
            active.len()
        );
        for (i, card) in active.iter().enumerate() {
            print_card_line(i + 1, card, &board.labels);
        }
    } else if let Some((list_partial, title)) = flag_values2(args, "--create") {
        let board = find_board(board_partial, by_id)?;
        let (lists, _all_cards) = lists_and_cards(&board);
        let list = find_list(&lists, list_partial, by_id)?.clone();
        let list_name = list.name.clone();
        let list_id = list.id.clone();
        let mut editor = BoardEditor::load(&board.id)?;
        editor.apply(Command::AddCard { list_id, title: title.to_string() })?;
        let new_id = editor
            .last_added_card_id()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("AddCard did not produce a new card id"))?;
        let card_title = editor
            .board()
            .cards
            .get(&new_id)
            .map(|c| c.title.clone())
            .unwrap_or_else(|| title.to_string());
        println!(
            "Created card '{}' in list '{}' on board '{}'.",
            card_title, list_name, board.name
        );
    } else if let Some(card_partial) = flag_value(args, "--edit") {
        edit(args, board_partial, card_partial, by_id)?;
    } else if let Some(card_partial) = flag_value(args, "--archive") {
        let board = find_board(board_partial, by_id)?;
        let (lists, all_cards) = lists_and_cards(&board);
        let (_, card) = find_card_in_lists(&lists, &all_cards, card_partial, false, by_id)?;
        let title = card.title.clone();
        let card_id = card.id.clone();
        let mut editor = BoardEditor::load(&board.id)?;
        editor.apply(Command::ArchiveCard { card_id })?;
        println!("Archived card '{title}'.");
    } else if let Some(card_partial) = flag_value(args, "--restore") {
        let board = find_board(board_partial, by_id)?;
        let card = find_archived_card(&board.id, card_partial, by_id)?;
        let title = card.title.clone();
        let card_id = card.id.clone();
        let mut editor = BoardEditor::load(&board.id)?;
        // Insert the archived card into the loaded board so RestoreCard
        // can find it; the editor will reattach it to a list.
        editor.with_extra_card(card);
        editor.apply(Command::RestoreCard { card_id })?;
        println!("Restored card '{title}'.");
    } else if let Some(card_partial) = flag_value(args, "--delete") {
        let board = find_board(board_partial, by_id)?;
        let card = find_archived_card(&board.id, card_partial, by_id)?;
        let title = card.title.clone();
        card_store::delete_card(&board.id, &card.id)?;
        println!("Permanently deleted card '{title}'.");
    } else {
        // Default: list all active cards grouped by list
        let board = find_board(board_partial, by_id)?;
        let (lists, all_cards) = lists_and_cards(&board);
        println!("Board: {} [{}]", board.name, board.id);
        for list in &lists {
            let active: Vec<_> = list
                .card_ids
                .iter()
                .filter_map(|id| all_cards.get(id))
                .filter(|c| !c.archived)
                .collect();
            println!(
                "  List: {} [{}]  ({} active cards)",
                list.name,
                list.id,
                active.len()
            );
            for (i, card) in active.iter().enumerate() {
                print_card_line(i + 1, card, &board.labels);
            }
        }
    }
    Ok(())
}

fn edit(args: &[String], board_partial: &str, card_partial: &str, by_id: bool) -> anyhow::Result<()> {
    let board = find_board(board_partial, by_id)?;
    let (lists, all_cards) = lists_and_cards(&board);
    let (_, card) = find_card_in_lists(&lists, &all_cards, card_partial, false, by_id)?;
    let card_id = card.id.clone();

    let new_title = flag_value(args, "--title");
    let new_desc = flag_value(args, "--description");
    let new_due = flag_value(args, "--due");

    if new_title.is_none() && new_desc.is_none() && new_due.is_none() {
        bail!("cards --edit requires at least one of: --title, --description, --due");
    }

    // Validate the due date before any mutation.
    let due_change: Option<Option<NaiveDate>> = match new_due {
        None => None,
        Some("none") => Some(None),
        Some(due_str) => {
            let d = NaiveDate::parse_from_str(due_str, "%Y-%m-%d")
                .context("Invalid date format. Use YYYY-MM-DD or 'none'.")?;
            Some(Some(d))
        }
    };

    let mut editor = BoardEditor::load(&board.id)?;
    if let Some(t) = new_title {
        editor.apply(Command::EditCardTitle {
            card_id: card_id.clone(),
            title: t.to_string(),
        })?;
    }
    if let Some(d) = new_desc {
        editor.apply(Command::EditCardDescription {
            card_id: card_id.clone(),
            body: d.to_string(),
        })?;
    }
    match due_change {
        Some(Some(d)) => editor.apply(Command::SetDueDate { card_id: card_id.clone(), date: d })?,
        Some(None) => editor.apply(Command::ClearDueDate { card_id: card_id.clone() })?,
        None => {}
    }
    let title = editor
        .board()
        .cards
        .get(&card_id)
        .map(|c| c.title.clone())
        .unwrap_or_default();
    println!("Updated card '{title}'.");
    Ok(())
}
