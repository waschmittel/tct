//! Insert mode key dispatcher.
//!
//! Routes the event to the active `Box<dyn InsertHandler>` on `App` and
//! interprets its [`InsertOutcome`] — applies commands, side effects,
//! and follow-up navigation.

use crossterm::event::KeyEvent;

use crate::app::App;
use crate::insert::{InsertOutcome, InsertSideEffect};
use crate::model::ids::ShortId;

/// Title of a card on the loaded board, for status messages.
fn card_title(app: &App, card_id: &ShortId) -> String {
    app.board()
        .and_then(|b| b.cards.get(card_id))
        .map(|c| c.title.clone())
        .unwrap_or_else(|| "card".into())
}

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
                crate::command::Command::EditCardTitle { title, .. } => {
                    Some(format!("Renamed card to '{title}'"))
                }
                crate::command::Command::SetDueDate { card_id, date } => {
                    Some(format!("Due date of '{}' set to {date}", card_title(app, card_id)))
                }
                crate::command::Command::ClearDueDate { card_id } => {
                    Some(format!("Cleared due date of '{}'", card_title(app, card_id)))
                }
                crate::command::Command::EditCardDescription { card_id, .. } => {
                    Some(format!("Saved description of '{}'", card_title(app, card_id)))
                }
                crate::command::Command::AddChecklistItem { text, .. } => {
                    Some(format!("Added item '{text}'"))
                }
                crate::command::Command::EditChecklistItem { text, .. } => {
                    Some(format!("Saved item '{text}'"))
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
        InsertOutcome::OpenDialogOverInsert(dialog) => {
            app.open_dialog_over_insert(dialog);
        }
        InsertOutcome::ConfirmSideEffect(eff) => {
            apply_side_effect(app, *eff)?;
            app.cancel_insert();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use crate::app::{App, AppMode};
    use crate::input::handle_input;
    use crate::model::card::Card;
    use crate::model::list::CardList;
    use crate::storage::{board_store, card_store, list_store};
    use crate::test_support::with_temp_dir;

    /// App parked in the description editor for the only card, with the
    /// editor's buffer already modified (so Esc triggers the discard prompt).
    fn app_editing_description() -> App {
        let mut meta = board_store::create_board("Test".into()).unwrap();
        let mut list = CardList::new("To Do".into());
        let card = Card::new("Card".into());
        card_store::save_card(&meta.id, &card).unwrap();
        list.card_ids.push(card.id.clone());
        list_store::save_list(&meta.id, &list).unwrap();
        meta.list_order = vec![list.id.clone()];
        board_store::save_board(&meta).unwrap();

        let mut app = App::new(Some(meta.id)).unwrap();
        app.mode = AppMode::CardDetail;
        // Open the description editor and modify it.
        press(&mut app, KeyCode::Char('e'));
        assert!(matches!(app.mode, AppMode::Insert));
        press(&mut app, KeyCode::Char('x'));
        app
    }

    fn press(app: &mut App, code: KeyCode) {
        handle_input(app, KeyEvent::new(code, KeyModifiers::empty())).unwrap();
    }

    fn is_markdown_editor(app: &App) -> bool {
        app.insert
            .as_ref()
            .and_then(|h| {
                h.as_any()
                    .downcast_ref::<crate::insert::markdown_editor::MarkdownEditor>()
            })
            .is_some()
    }

    #[test]
    fn esc_with_unsaved_changes_opens_dialog_and_keeps_editor_alive() {
        with_temp_dir(|| {
            let mut app = app_editing_description();
            press(&mut app, KeyCode::Esc);
            // Dialog is shown, but the editor handler must stay alive so a
            // resume can return to it.
            assert!(matches!(app.mode, AppMode::Dialog));
            assert!(app.dialog.is_some());
            assert!(is_markdown_editor(&app), "editor handler dropped");
        });
    }

    #[test]
    fn resume_returns_to_a_live_editor_not_a_frozen_insert_mode() {
        with_temp_dir(|| {
            let mut app = app_editing_description();
            press(&mut app, KeyCode::Esc);
            // "n" = keep editing.
            press(&mut app, KeyCode::Char('n'));
            assert!(matches!(app.mode, AppMode::Insert));
            assert!(is_markdown_editor(&app), "resumed Insert mode has no handler");
            // Regression guard: a key must still be handled (not frozen).
            press(&mut app, KeyCode::Char('z'));
            assert!(matches!(app.mode, AppMode::Insert));
            assert!(is_markdown_editor(&app));
        });
    }

    #[test]
    fn discard_returns_to_card_detail_not_a_frozen_insert_mode() {
        with_temp_dir(|| {
            let mut app = app_editing_description();
            press(&mut app, KeyCode::Esc);
            // "y" = discard changes.
            press(&mut app, KeyCode::Char('y'));
            assert!(
                matches!(app.mode, AppMode::CardDetail),
                "discard should return to CardDetail, got {:?}",
                app.mode
            );
            assert!(app.insert.is_none(), "editor handler should be gone");
            assert!(app.dialog.is_none());
            // Regression guard: card detail still responds.
            press(&mut app, KeyCode::Esc);
            assert!(matches!(app.mode, AppMode::Normal));
        });
    }
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
