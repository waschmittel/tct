//! `tct labels <board>` subcommand: list/create/delete/assign/remove board labels.

use super::lookup::{find_board, find_card_in_lists, find_label};
use super::util::{flag_value, flag_values2, label_color_name, lists_and_cards};
use crate::board_editor::BoardEditor;
use crate::command::Command;
use crate::model::label::LabelColor;

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
        let board = find_board(board_partial, by_id)?;
        let board_name = board.name.clone();
        let existing: Vec<_> = board.labels.iter().map(|l| l.color).collect();
        let color = LabelColor::generate_pastel(&existing);
        let mut editor = BoardEditor::load(&board.id)?;
        editor.apply(Command::DefineLabel { name: name.to_string(), color })?;
        println!("Created label '{name}' on board '{}'.", board_name);
    } else if let Some(label_partial) = flag_value(args, "--delete") {
        let board = find_board(board_partial, by_id)?;
        let board_name = board.name.clone();
        let label = find_label(&board.labels, label_partial, by_id)?.clone();
        let label_name = label.name.clone();
        let mut editor = BoardEditor::load(&board.id)?;
        editor.apply(Command::DeleteLabel { label_id: label.id })?;
        println!("Deleted label '{label_name}' from board '{}'.", board_name);
    } else if let Some((card_partial, label_partial)) = flag_values2(args, "--assign") {
        let board = find_board(board_partial, by_id)?;
        let label = find_label(&board.labels, label_partial, by_id)?.clone();
        let (lists, all_cards) = lists_and_cards(&board);
        let (_, card) = find_card_in_lists(&lists, &all_cards, card_partial, false, by_id)?;
        if card.label_ids.contains(&label.id) {
            println!(
                "Label '{}' already assigned to card '{}'.",
                label.name, card.title
            );
        } else {
            let card_title = card.title.clone();
            let label_name = label.name.clone();
            let mut editor = BoardEditor::load(&board.id)?;
            editor.apply(Command::ToggleLabel {
                card_id: card.id.clone(),
                label_id: label.id.clone(),
            })?;
            println!("Assigned label '{label_name}' to card '{card_title}'.");
        }
    } else if let Some((card_partial, label_partial)) = flag_values2(args, "--remove") {
        let board = find_board(board_partial, by_id)?;
        let label = find_label(&board.labels, label_partial, by_id)?.clone();
        let (lists, all_cards) = lists_and_cards(&board);
        let (_, card) = find_card_in_lists(&lists, &all_cards, card_partial, false, by_id)?;
        if !card.label_ids.contains(&label.id) {
            println!(
                "Label '{}' is not assigned to card '{}'.",
                label.name, card.title
            );
        } else {
            let card_title = card.title.clone();
            let label_name = label.name.clone();
            let mut editor = BoardEditor::load(&board.id)?;
            editor.apply(Command::ToggleLabel {
                card_id: card.id.clone(),
                label_id: label.id.clone(),
            })?;
            println!("Removed label '{label_name}' from card '{card_title}'.");
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
