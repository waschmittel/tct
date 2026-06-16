//! `tct checklist <board> <card>` subcommand.

use anyhow::{bail, Context};

use super::lookup::{find_board, find_card_in_lists};
use super::util::{flag_value, fmt_progress, lists_and_cards};
use crate::board_editor::BoardEditor;
use crate::command::Command;

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
    let (lists, all_cards) = lists_and_cards(&board);

    if let Some(text) = flag_value(args, "--add") {
        let (_, card) = find_card_in_lists(&lists, &all_cards, card_partial, false, by_id)?;
        let card_id = card.id.clone();
        let card_title = card.title.clone();
        let mut editor = BoardEditor::load(&board.id)?;
        editor.apply(Command::AddChecklistItem { card_id, text: text.to_string() })?;
        println!("Added checklist item '{}' to card '{}'.", text, card_title);
    } else if let Some(n_str) = flag_value(args, "--toggle") {
        let n: usize = n_str
            .parse()
            .context("Item index must be a positive integer")?;
        let (_, card) = find_card_in_lists(&lists, &all_cards, card_partial, false, by_id)?;
        let idx = n
            .checked_sub(1)
            .ok_or_else(|| anyhow::anyhow!("Index must be >= 1"))?;
        let total = card.checklist.len();
        if idx >= total {
            bail!("Index {n} out of range (card has {total} items)");
        }
        let item_text = card.checklist[idx].text.clone();
        let card_id = card.id.clone();
        let mut editor = BoardEditor::load(&board.id)?;
        editor.apply(Command::ToggleChecklistItem { card_id: card_id.clone(), item_idx: idx })?;
        let new_state = editor
            .board()
            .cards
            .get(&card_id)
            .and_then(|c| c.checklist.get(idx))
            .map(|i| i.completed)
            .unwrap_or(false);
        let state = if new_state { "done" } else { "undone" };
        println!("Toggled item {n} ('{item_text}') → {state}.");
    } else if let Some(n_str) = flag_value(args, "--delete") {
        let n: usize = n_str
            .parse()
            .context("Item index must be a positive integer")?;
        let (_, card) = find_card_in_lists(&lists, &all_cards, card_partial, false, by_id)?;
        let idx = n
            .checked_sub(1)
            .ok_or_else(|| anyhow::anyhow!("Index must be >= 1"))?;
        if idx >= card.checklist.len() {
            bail!(
                "Index {n} out of range (card has {} items)",
                card.checklist.len()
            );
        }
        let removed_text = card.checklist[idx].text.clone();
        let card_id = card.id.clone();
        let mut editor = BoardEditor::load(&board.id)?;
        editor.apply(Command::RemoveChecklistItem { card_id, item_idx: idx })?;
        println!("Deleted checklist item '{removed_text}'.");
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
