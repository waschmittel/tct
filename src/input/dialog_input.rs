//! Dialog mode key dispatcher.
//!
//! Routes the event to the active `Box<dyn Dialog>` on `App` and
//! interprets its [`DialogOutcome`] — applies commands, side effects,
//! status messages, and follow-up navigation.

use crossterm::event::KeyEvent;

use crate::app::{App, AppMode};
use crate::dialog::{DialogSideEffect, Follow};
use crate::insert::line_editor;
use crate::storage::{board_store, card_store, list_store};

pub fn handle(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    // Temporarily move the dialog out so we can pass a borrow of `board`
    // alongside a mutable borrow of the dialog itself.
    let mut dialog = match app.dialog.take() {
        Some(d) => d,
        None => return Ok(()),
    };
    let outcome = dialog.handle_key(key, app.board.as_ref());
    // Put the dialog back so subsequent ops (apply, side effects) can
    // see consistent state. Follow may replace or remove it below.
    app.dialog = Some(dialog);

    if let Some(cmd) = outcome.apply {
        app.apply(cmd)?;
    }
    if let Some(eff) = outcome.side_effect {
        apply_side_effect(app, eff)?;
    }
    if let Some(status) = outcome.status {
        app.set_status(status);
    }
    match outcome.follow {
        Follow::Stay => {}
        Follow::Close => {
            app.close_dialog();
        }
        Follow::CloseTo(target) => {
            app.close_dialog_to(target);
        }
        Follow::Open(next) => {
            app.dialog = Some(next);
            // Mode stays AppMode::Dialog.
        }
    }
    Ok(())
}

fn apply_side_effect(app: &mut App, eff: DialogSideEffect) -> anyhow::Result<()> {
    match eff {
        DialogSideEffect::DeleteArchivedBoard { board_id } => {
            board_store::delete_board(&board_id)?;
        }
        DialogSideEffect::DeleteArchivedList { list_id, card_ids } => {
            if let Some(board) = &app.board {
                for cid in &card_ids {
                    let _ = card_store::delete_card(&board.meta.id, cid);
                }
                list_store::delete_list_file(&board.meta.id, &list_id)?;
            }
        }
        DialogSideEffect::DeleteArchivedCard { card_id } => {
            if let Some(board) = &app.board {
                card_store::delete_card(&board.meta.id, &card_id)?;
            }
        }
        DialogSideEffect::RestoreArchivedBoard { board_id } => {
            let mut editor = crate::board_editor::BoardEditor::load(&board_id)?;
            editor.apply(crate::command::Command::RestoreBoard {
                board_id: board_id.clone(),
            })?;
            board_store::append_to_order(&board_id)?;
            app.reload_boards()?;
        }
        DialogSideEffect::ArchiveSelectedBoard => {
            if let Some(board) = app.boards.get(app.selected_board_idx) {
                let id = board.id.clone();
                let mut editor = crate::board_editor::BoardEditor::load(&id)?;
                editor.apply(crate::command::Command::ArchiveBoard {
                    board_id: id.clone(),
                })?;
                board_store::remove_from_order(&id)?;
                app.reload_boards()?;
                if app.selected_board_idx > 0 && app.selected_board_idx >= app.boards.len() {
                    app.selected_board_idx = app.boards.len().saturating_sub(1);
                }
            }
        }
        DialogSideEffect::StageAndRestoreCard { card } => {
            let card_id = card.id.clone();
            if let Some(board) = &mut app.board {
                board.cards.insert(card_id.clone(), card);
            }
            app.apply(crate::command::Command::RestoreCard { card_id })?;
        }
        DialogSideEffect::DiscardDescriptionEdit => {
            app.insert = None;
            app.dialog = None;
            app.mode = app.previous_mode.take().unwrap_or(AppMode::CardDetail);
        }
        DialogSideEffect::ResumeDescriptionEdit => {
            // The MarkdownEditor handler is still on `app.insert` — just
            // switch back to Insert mode.
            app.dialog = None;
            app.mode = AppMode::Insert;
        }
        DialogSideEffect::ReorderLabels { from, to } => {
            if let Some(board) = &mut app.board
                && from < board.meta.labels.len()
                && to < board.meta.labels.len()
            {
                board.meta.labels.swap(from, to);
                board_store::save_board(&board.meta)?;
            }
        }
        DialogSideEffect::StartNewLabelInsert => {
            // Close the LabelManager and start NewLabelName insert. The
            // confirm handler reopens LabelManager.
            app.dialog = None;
            app.start_insert(Box::new(line_editor::NewLabelName::new()));
        }
        DialogSideEffect::StartRenameLabelInsert {
            label_idx,
            current_name,
        } => {
            let label_id = app
                .board
                .as_ref()
                .and_then(|b| b.meta.labels.get(label_idx).map(|l| l.id.clone()));
            if let Some(label_id) = label_id {
                app.dialog = None;
                app.start_insert(Box::new(line_editor::EditLabelName::new(
                    label_id,
                    label_idx,
                    &current_name,
                )));
            }
        }
    }
    Ok(())
}
