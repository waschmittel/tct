//! Insert mode key dispatcher.
//!
//! Routes the event to the active `Box<dyn InsertHandler>` on `App` and
//! interprets its [`InsertOutcome`] — applies commands, side effects,
//! and follow-up navigation.

use crossterm::event::KeyEvent;

use crate::app::App;
use crate::insert::{InsertOutcome, InsertSideEffect};
use crate::storage::board_store;

pub fn handle(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    // Move the handler out so it can be `&mut` while we borrow `board`.
    let mut handler = match app.insert.take() {
        Some(h) => h,
        None => return Ok(()),
    };
    let outcome = handler.handle_key(key, app.board.as_ref());
    app.insert = Some(handler);

    match outcome {
        InsertOutcome::Stay => {}
        InsertOutcome::Cancel => {
            app.cancel_insert();
        }
        InsertOutcome::CancelWithStatus(msg) => {
            app.set_status(msg);
        }
        InsertOutcome::Confirm(cmd) => {
            // Card title changes need a status update for the user.
            let status = match &cmd {
                crate::command::Command::AddCard { title, .. } => {
                    Some(format!("Added card '{title}'"))
                }
                crate::command::Command::AddList { name } => {
                    Some(format!("Added list '{name}'"))
                }
                crate::command::Command::RenameList { name, .. } => {
                    Some(format!("Renamed list to '{name}'"))
                }
                crate::command::Command::SetDueDate { date, .. } => {
                    Some(format!("Due date set to {date}"))
                }
                crate::command::Command::ClearDueDate { .. } => {
                    Some("Cleared due date".into())
                }
                crate::command::Command::EditCardDescription { .. } => {
                    Some("Description saved".into())
                }
                _ => None,
            };
            // Special: AddCard / AddList should also move selection.
            let was_add_card =
                matches!(cmd, crate::command::Command::AddCard { .. });
            let was_add_list =
                matches!(cmd, crate::command::Command::AddList { .. });
            let was_add_checklist =
                matches!(cmd, crate::command::Command::AddChecklistItem { .. });
            // Extract the card id for AddChecklistItem so we can move selection
            // after the command applies. The card_id is shared via the handler.
            let add_checklist_card_id = if let crate::command::Command::AddChecklistItem {
                card_id, ..
            } = &cmd
            {
                Some(card_id.clone())
            } else {
                None
            };

            app.apply(cmd)?;

            // Post-confirm selection moves.
            if was_add_card
                && let Some(b) = &mut app.board
            {
                let li = b.selected_list;
                if let Some(list) = b.lists.get(li) {
                    b.selected_card[li] = list.card_ids.len().saturating_sub(1);
                }
            }
            if was_add_list
                && let Some(b) = &mut app.board
            {
                b.selected_list = b.lists.len().saturating_sub(1);
            }
            if was_add_checklist
                && let Some(card_id) = add_checklist_card_id
                && let Some(b) = &mut app.board
                && let Some(c) = b.cards.get(&card_id)
            {
                b.detail_item_idx = c.checklist.len().saturating_sub(1);
            }
            if let Some(s) = status {
                app.set_status(s);
            }
            app.cancel_insert();
        }
        InsertOutcome::ConfirmAndOpenDialog(cmd, dialog) => {
            let status = match &cmd {
                crate::command::Command::DefineLabel { name, .. } => {
                    Some(format!("Created label '{name}'"))
                }
                _ => None,
            };
            app.apply(cmd)?;
            if let Some(s) = status {
                app.set_status(s);
            }
            app.insert = None;
            app.open_dialog(dialog);
        }
        InsertOutcome::OpenDialog(dialog) => {
            app.insert = None;
            app.open_dialog(dialog);
        }
        InsertOutcome::ConfirmSideEffect(eff) => {
            apply_side_effect(app, *eff)?;
            app.cancel_insert();
        }
    }
    Ok(())
}

fn apply_side_effect(app: &mut App, eff: InsertSideEffect) -> anyhow::Result<()> {
    match eff {
        InsertSideEffect::CreateBoard { name } => {
            let existing_colors: Vec<_> = app.boards.iter().map(|b| b.accent_color).collect();
            let mut meta = crate::model::board::BoardMeta::new(name.clone());
            meta.accent_color =
                crate::model::label::LabelColor::generate_pastel(&existing_colors);
            board_store::save_board(&meta)?;
            board_store::append_to_order(&meta.id)?;
            app.reload_boards()?;
            app.set_status(format!("Created board '{name}'"));
        }
        InsertSideEffect::RenameSelectedBoard { new_name } => {
            if let Some(board) = app.boards.get(app.selected_board_idx) {
                let id = board.id.clone();
                let mut editor = crate::board_editor::BoardEditor::load(&id)?;
                editor.apply(crate::command::Command::RenameBoard {
                    name: new_name.clone(),
                })?;
                if let Some(b) = app.boards.get_mut(app.selected_board_idx) {
                    b.name = new_name.clone();
                }
                app.set_status(format!("Renamed board to '{new_name}'"));
            }
        }
    }
    Ok(())
}
