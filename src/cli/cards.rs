//! `tct cards <board>` subcommand: list/show/create/edit/archive/restore/delete cards.

use anyhow::{bail, Context};
use chrono::NaiveDate;

use super::lookup::{find_archived_card, find_board, find_card_in_lists, find_list};
use super::util::{
    flag_value, flag_values2, has_flag, load_all_cards, print_card_detail, print_card_line,
};
use crate::model::card::Card;
use crate::storage::{board_store, card_store, list_store};

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
        let lists = list_store::load_all_lists(&board.id, &board.list_order)?;
        let all_cards = load_all_cards(&board.id, &lists);
        let (list, card) = find_card_in_lists(&lists, &all_cards, card_partial, false, by_id)?;
        print_card_detail(&card, &board, &list);
    } else if let Some(list_partial) = flag_value(args, "--list") {
        let board = find_board(board_partial, by_id)?;
        let lists = list_store::load_all_lists(&board.id, &board.list_order)?;
        let list = find_list(&lists, list_partial, by_id)?.clone();
        let all_cards = load_all_cards(&board.id, &lists);
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
        let lists = list_store::load_all_lists(&board.id, &board.list_order)?;
        let mut list = find_list(&lists, list_partial, by_id)?.clone();
        let mut card = Card::new(title.to_string());
        card.log("Created");
        card_store::save_card(&board.id, &card)?;
        list.card_ids.push(card.id.clone());
        list_store::save_list(&board.id, &list)?;
        println!(
            "Created card '{}' in list '{}' on board '{}'.",
            card.title, list.name, board.name
        );
    } else if let Some(card_partial) = flag_value(args, "--edit") {
        edit(args, board_partial, card_partial, by_id)?;
    } else if let Some(card_partial) = flag_value(args, "--archive") {
        let board = find_board(board_partial, by_id)?;
        let lists = list_store::load_all_lists(&board.id, &board.list_order)?;
        let all_cards = load_all_cards(&board.id, &lists);
        let (mut list, mut card) =
            find_card_in_lists(&lists, &all_cards, card_partial, false, by_id)?;
        let title = card.title.clone();
        card.archived = true;
        card.log("Archived");
        card_store::save_card(&board.id, &card)?;
        list.card_ids.retain(|id| id != &card.id);
        list_store::save_list(&board.id, &list)?;
        println!("Archived card '{title}'.");
    } else if let Some(card_partial) = flag_value(args, "--restore") {
        let board = find_board(board_partial, by_id)?;
        let mut card = find_archived_card(&board.id, card_partial, by_id)?;
        let title = card.title.clone();
        card.archived = false;
        card.log("Restored from archive");
        card_store::save_card(&board.id, &card)?;
        let meta = board_store::load_board(&board.id)?;
        if let Some(first_list_id) = meta.list_order.first()
            && let Ok(mut list) = list_store::load_list(&board.id, first_list_id)
        {
            list.card_ids.push(card.id.clone());
            list_store::save_list(&board.id, &list)?;
        }
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
        let lists = list_store::load_all_lists(&board.id, &board.list_order)?;
        let all_cards = load_all_cards(&board.id, &lists);
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
    let lists = list_store::load_all_lists(&board.id, &board.list_order)?;
    let all_cards = load_all_cards(&board.id, &lists);
    let (_, mut card) = find_card_in_lists(&lists, &all_cards, card_partial, false, by_id)?;

    let new_title = flag_value(args, "--title");
    let new_desc = flag_value(args, "--description");
    let new_due = flag_value(args, "--due");

    if new_title.is_none() && new_desc.is_none() && new_due.is_none() {
        bail!("cards --edit requires at least one of: --title, --description, --due");
    }
    let mut actions: Vec<String> = Vec::new();
    if let Some(t) = new_title {
        if card.title != t {
            actions.push("Edited title".into());
        }
        card.title = t.to_string();
    }
    if let Some(d) = new_desc {
        if card.description != d {
            actions.push("Edited description".into());
        }
        card.description = d.to_string();
    }
    if let Some(due_str) = new_due {
        if due_str == "none" {
            if card.due_date.is_some() {
                actions.push("Cleared due date".into());
            }
            card.due_date = None;
        } else {
            let d = NaiveDate::parse_from_str(due_str, "%Y-%m-%d")
                .context("Invalid date format. Use YYYY-MM-DD or 'none'.")?;
            if card.due_date != Some(d) {
                actions.push(format!("Set due date to {d}"));
            }
            card.due_date = Some(d);
        }
    }
    if actions.is_empty() {
        card.touch();
    } else {
        for a in actions {
            card.log(a);
        }
    }
    card_store::save_card(&board.id, &card)?;
    println!("Updated card '{}'.", card.title);
    Ok(())
}
