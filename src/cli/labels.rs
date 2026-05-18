//! `tct labels <board>` subcommand: list/create/delete/assign/remove board labels.

use super::lookup::{find_board, find_card_in_lists, find_label};
use super::util::{flag_value, flag_values2, label_color_name, load_all_cards};
use crate::model::label::{Label, LabelColor};
use crate::storage::{board_store, card_store, list_store};

pub(super) fn run(args: &[String], by_id: bool) -> anyhow::Result<()> {
    let board_partial = args
        .first()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Usage: tct labels <board> [--create|--delete|--assign|--remove ...]"
            )
        })?
        .as_str();

    if let Some(name) = flag_value(args, "--create") {
        let mut board = find_board(board_partial, by_id)?;
        let existing: Vec<_> = board.labels.iter().map(|l| l.color).collect();
        let color = LabelColor::generate_pastel(&existing);
        let label = Label::new(name.to_string(), color);
        board.labels.push(label);
        board_store::save_board(&board)?;
        println!("Created label '{name}' on board '{}'.", board.name);
    } else if let Some(label_partial) = flag_value(args, "--delete") {
        let mut board = find_board(board_partial, by_id)?;
        let label = find_label(&board.labels, label_partial, by_id)?.clone();
        let label_name = label.name.clone();
        board.labels.retain(|l| l.id != label.id);
        let lists = list_store::load_all_lists(&board.id, &board.list_order)?;
        for list in &lists {
            for card_id in &list.card_ids {
                if let Ok(mut card) = card_store::load_card(&board.id, card_id)
                    && card.label_ids.contains(&label.id)
                {
                    card.label_ids.retain(|id| id != &label.id);
                    card.log(format!("Removed label '{label_name}'"));
                    let _ = card_store::save_card(&board.id, &card);
                }
            }
        }
        board_store::save_board(&board)?;
        println!("Deleted label '{label_name}' from board '{}'.", board.name);
    } else if let Some((card_partial, label_partial)) = flag_values2(args, "--assign") {
        let board = find_board(board_partial, by_id)?;
        let label = find_label(&board.labels, label_partial, by_id)?.clone();
        let lists = list_store::load_all_lists(&board.id, &board.list_order)?;
        let all_cards = load_all_cards(&board.id, &lists);
        let (_, mut card) = find_card_in_lists(&lists, &all_cards, card_partial, false, by_id)?;
        if !card.label_ids.contains(&label.id) {
            card.label_ids.push(label.id.clone());
            card.log(format!("Added label '{}'", label.name));
            card_store::save_card(&board.id, &card)?;
            println!("Assigned label '{}' to card '{}'.", label.name, card.title);
        } else {
            println!(
                "Label '{}' already assigned to card '{}'.",
                label.name, card.title
            );
        }
    } else if let Some((card_partial, label_partial)) = flag_values2(args, "--remove") {
        let board = find_board(board_partial, by_id)?;
        let label = find_label(&board.labels, label_partial, by_id)?.clone();
        let lists = list_store::load_all_lists(&board.id, &board.list_order)?;
        let all_cards = load_all_cards(&board.id, &lists);
        let (_, mut card) = find_card_in_lists(&lists, &all_cards, card_partial, false, by_id)?;
        if card.label_ids.contains(&label.id) {
            card.label_ids.retain(|id| id != &label.id);
            card.log(format!("Removed label '{}'", label.name));
            card_store::save_card(&board.id, &card)?;
            println!("Removed label '{}' from card '{}'.", label.name, card.title);
        } else {
            println!(
                "Label '{}' is not assigned to card '{}'.",
                label.name, card.title
            );
        }
    } else {
        // Default: list labels
        let board = find_board(board_partial, by_id)?;
        println!(
            "Board: {} [{}]  Labels ({}):",
            board.name,
            board.id,
            board.labels.len()
        );
        if board.labels.is_empty() {
            println!("  (no labels)");
        } else {
            for label in &board.labels {
                println!(
                    "  [{}]  {}  ({})",
                    label.id,
                    label.name,
                    label_color_name(&label.color)
                );
            }
        }
    }
    Ok(())
}
