//! `tct checklist <board> <card>` subcommand.

use anyhow::{bail, Context};

use super::lookup::{find_board, find_card_in_lists};
use super::util::{flag_value, fmt_progress, load_all_cards};
use crate::model::card::ChecklistItem;
use crate::storage::{card_store, list_store};

pub(super) fn run(args: &[String], by_id: bool) -> anyhow::Result<()> {
    let board_partial = args
        .first()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Usage: tct checklist <board> <card> [--add|--toggle|--delete ...]"
            )
        })?
        .as_str();
    let card_partial = args
        .get(1)
        .ok_or_else(|| anyhow::anyhow!("Missing required argument: card name"))?
        .as_str();

    let board = find_board(board_partial, by_id)?;
    let lists = list_store::load_all_lists(&board.id, &board.list_order)?;
    let all_cards = load_all_cards(&board.id, &lists);

    if let Some(text) = flag_value(args, "--add") {
        let (_, mut card) = find_card_in_lists(&lists, &all_cards, card_partial, false, by_id)?;
        card.checklist.push(ChecklistItem {
            text: text.to_string(),
            completed: false,
        });
        card.log(format!("Added checklist item '{text}'"));
        card_store::save_card(&board.id, &card)?;
        println!("Added checklist item '{}' to card '{}'.", text, card.title);
    } else if let Some(n_str) = flag_value(args, "--toggle") {
        let n: usize = n_str
            .parse()
            .context("Item index must be a positive integer")?;
        let (_, mut card) = find_card_in_lists(&lists, &all_cards, card_partial, false, by_id)?;
        let idx = n
            .checked_sub(1)
            .ok_or_else(|| anyhow::anyhow!("Index must be >= 1"))?;
        let total = card.checklist.len();
        let item = card
            .checklist
            .get_mut(idx)
            .ok_or_else(|| anyhow::anyhow!("Index {n} out of range (card has {total} items)"))?;
        item.completed = !item.completed;
        let state = if item.completed { "done" } else { "undone" };
        let text = item.text.clone();
        let action = if item.completed {
            format!("Completed checklist item '{text}'")
        } else {
            format!("Uncompleted checklist item '{text}'")
        };
        card.log(action);
        card_store::save_card(&board.id, &card)?;
        println!("Toggled item {n} ('{text}') → {state}.");
    } else if let Some(n_str) = flag_value(args, "--delete") {
        let n: usize = n_str
            .parse()
            .context("Item index must be a positive integer")?;
        let (_, mut card) = find_card_in_lists(&lists, &all_cards, card_partial, false, by_id)?;
        let idx = n
            .checked_sub(1)
            .ok_or_else(|| anyhow::anyhow!("Index must be >= 1"))?;
        if idx >= card.checklist.len() {
            bail!(
                "Index {n} out of range (card has {} items)",
                card.checklist.len()
            );
        }
        let removed = card.checklist.remove(idx);
        card.log(format!("Removed checklist item '{}'", removed.text));
        card_store::save_card(&board.id, &card)?;
        println!("Deleted checklist item '{}'.", removed.text);
    } else {
        // Default: show checklist
        let (_, card) = find_card_in_lists(&lists, &all_cards, card_partial, false, by_id)?;
        println!("Card: {}  Checklist: {}", card.title, fmt_progress(&card));
        if card.checklist.is_empty() {
            println!("  (no items)");
        } else {
            for (i, item) in card.checklist.iter().enumerate() {
                let mark = if item.completed { "x" } else { " " };
                println!("  {}. [{}] {}", i + 1, mark, item.text);
            }
        }
    }
    Ok(())
}
