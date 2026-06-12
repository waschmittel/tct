//! Insert mode key dispatcher.
//!
//! Routes the event to the active `Box<dyn InsertHandler>` on `App` and
//! interprets its [`InsertOutcome`] — applies commands, side effects,
//! and follow-up navigation.

use crossterm::event::KeyEvent;

use crate::app::App;
use crate::insert::{InsertOutcome, InsertSideEffect};

pub fn handle(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    // Move the handler out so it can be `&mut` while we borrow `board`.
    let mut handler = match app.insert.take() {
        Some(h) => h,
        None => return Ok(()),
    };
    let outcome = handler.handle_key(key, app.board());
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
                crate::command::Command::EditCardTitle { .. } => {
                    Some("Title saved".into())
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
                crate::command::Command::AddChecklistItem { .. } => {
                    Some("Item added".into())
                }
                crate::command::Command::EditChecklistItem { .. } => {
                    Some("Item saved".into())
                }
                _ => None,
            };
            // Selection-follow for Add* commands happens inside
            // BoardEditor::apply.
            app.apply(cmd)?;
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
            let meta = crate::board_directory::create(name)?;
            app.reload_boards()?;
            app.set_status(format!("Created board '{}'", meta.name));
        }
        InsertSideEffect::RenameSelectedBoard { new_name } => {
            if let Some(board) = app.boards.get(app.selected_board_idx) {
                let id = board.id.clone();
                crate::board_directory::rename(&id, new_name.clone())?;
                if let Some(b) = app.boards.get_mut(app.selected_board_idx) {
                    b.name = new_name.clone();
                }
                app.set_status(format!("Renamed board to '{new_name}'"));
            }
        }
    }
    Ok(())
}
