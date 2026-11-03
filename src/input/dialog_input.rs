//! Dialog mode key dispatcher.
//!
//! Routes the event to the active `Box<dyn Dialog>` on `App` and
//! interprets its [`DialogOutcome`] — applies commands, side effects,
//! status messages, and follow-up navigation.

use crossterm::event::KeyEvent;

use crate::app::{App, AppMode};
use crate::dialog::{DialogSideEffect, Follow};
use crate::insert::line_editor;

pub fn handle(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    // Temporarily move the dialog out so we can pass a borrow of `board`
    // alongside a mutable borrow of the dialog itself.
    let mut dialog = match app.dialog.take() {
        Some(d) => d,
        None => return Ok(()),
    };
    let outcome = dialog.handle_key(key, app.board());
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
        Follow::Help => {
            // Dialog stays alive on `app.dialog`; Help-Esc returns to it.
            app.mode = AppMode::Help;
        }
    }
    Ok(())
}

/// Export failures are shown as a toast; they are not worth crashing the TUI for.
fn export_card_pdf(app: &mut App, card_id: &crate::model::id::CardId, path: &std::path::Path) {
    let Some(board) = app.board() else { return };
    let Some(card) = board.cards.get(card_id) else { return };
    let list = board
        .lists
        .iter()
        .find(|l| l.id == card.list_id)
        .map_or("", |l| l.name.as_str());
    let written = crate::export::card_pdf(&board.meta, list, card)
        .and_then(|pdf| std::fs::write(path, pdf).map_err(Into::into));
    match written {
        Ok(()) => {
            app.export_dir = path.parent().map(std::path::Path::to_path_buf);
            app.set_status(format!("Exported PDF to {}", path.display()));
        }
        Err(e) => app.set_status(format!("Export failed: {e}")),
    }
}

fn apply_side_effect(app: &mut App, eff: DialogSideEffect) -> anyhow::Result<()> {
    match eff {
        DialogSideEffect::DeleteArchivedBoard { board_id } => {
            crate::board_directory::delete(&board_id)?;
        }
        DialogSideEffect::DeleteArchivedList { list_id, card_ids } => {
            if let Some(editor) = &mut app.editor {
                editor.delete_archived_list(&list_id, &card_ids)?;
            }
        }
        DialogSideEffect::ExportCardPdf { card_id, path } => export_card_pdf(app, &card_id, &path),
        DialogSideEffect::DeleteArchivedCard { card_id } => {
            if let Some(editor) = &mut app.editor {
                editor.delete_archived_card(&card_id)?;
            }
        }
        DialogSideEffect::RestoreArchivedBoard { board_id } => {
            crate::board_directory::restore(&board_id)?;
            app.reload_boards()?;
        }
        DialogSideEffect::ArchiveSelectedBoard => {
            if let Some(board) = app.boards.get(app.selected_board_idx) {
                let id = board.id.clone();
                crate::board_directory::archive(&id)?;
                app.reload_boards()?;
                if app.selected_board_idx > 0 && app.selected_board_idx >= app.boards.len() {
                    app.selected_board_idx = app.boards.len().saturating_sub(1);
                }
            }
        }
        DialogSideEffect::ConfigureSelectedBoard { color, new_card_position } => {
            if let Some(board) = app.boards.get(app.selected_board_idx) {
                let id = board.id.clone();
                crate::board_directory::configure_board(&id, color, new_card_position)?;
                if let Some(b) = app.boards.get_mut(app.selected_board_idx) {
                    b.accent_color = color;
                    b.new_card_position = new_card_position;
                }
                if let Some(editor) = &mut app.editor {
                    if editor.board().meta.id == id {
                        editor.reload()?;
                    }
                }
            }
        }
        DialogSideEffect::StageAndRestoreCard { card } => {
            let card_id = card.id.clone();
            if let Some(editor) = &mut app.editor {
                editor.with_extra_card(card);
            }
            app.apply(crate::command::Command::RestoreCard { card_id })?;
        }
        DialogSideEffect::DiscardDescriptionEdit => {
            app.insert = None;
            app.dialog = None;
            app.mode = app.take_return_mode();
        }
        DialogSideEffect::ResumeDescriptionEdit => {
            // The MarkdownEditor handler is still on `app.insert` — just
            // switch back to Insert mode.
            app.dialog = None;
            app.mode = AppMode::Insert;
        }
        DialogSideEffect::ReorderLabels { from, to } => {
            app.apply(crate::command::Command::ReorderLabels { from, to })?;
        }
        DialogSideEffect::StartNewLabelInsert { from_picker } => {
            // Close the LabelManager and start NewLabelName insert. The
            // confirm handler reopens LabelManager.
            app.dialog = None;
            app.start_insert(Box::new(line_editor::NewLabelName::new(from_picker)));
        }
        DialogSideEffect::StartRenameLabelInsert {
            label_idx,
            current_name,
            from_picker,
        } => {
            let label_id = app
                .board()
                .and_then(|b| b.meta.labels.get(label_idx).map(|l| l.id.clone()));
            if let Some(label_id) = label_id {
                app.dialog = None;
                app.start_insert(Box::new(line_editor::EditLabelName::new(
                    label_id,
                    label_idx,
                    &current_name,
                    from_picker,
                )));
            }
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
    use crate::storage::board_store;
    use crate::test_support::with_temp_dir;

    fn press(app: &mut App, code: KeyCode) {
        handle_input(app, KeyEvent::new(code, KeyModifiers::empty())).unwrap();
    }

    fn app_with_card(title: &str) -> App {
        let mut meta = board_store::create_board("Test".into()).unwrap();
        let list = crate::test_support::add_list(&mut meta, "To Do");
        let mut card = Card::new(title.into());
        crate::test_support::put_cards(&meta.id, &list, std::slice::from_mut(&mut card));
        App::new(Some(meta.id)).unwrap()
    }

    #[test]
    fn p_exports_the_card_and_the_next_export_starts_in_the_same_folder() {
        with_temp_dir(|| {
            let out = tempfile::tempdir().unwrap();
            let mut app = app_with_card("First card");
            app.export_dir = Some(out.path().to_path_buf());

            press(&mut app, KeyCode::Char('p'));
            assert_eq!(app.mode, AppMode::Dialog);
            press(&mut app, KeyCode::Enter);
            assert_eq!(app.mode, AppMode::Normal);

            let file = out.path().join("first-card.pdf");
            assert!(std::fs::read(&file).unwrap().starts_with(b"%PDF-"));
            assert_eq!(app.export_dir.as_deref(), Some(out.path()));
            let status = &app.status_message.as_ref().unwrap().0;
            assert!(status.starts_with("Exported PDF to "), "{status}");
        });
    }

    #[test]
    fn export_dialog_opens_from_card_detail_too() {
        with_temp_dir(|| {
            let mut app = app_with_card("Card");
            press(&mut app, KeyCode::Enter);
            assert_eq!(app.mode, AppMode::CardDetail);
            press(&mut app, KeyCode::Char('p'));
            assert_eq!(app.mode, AppMode::Dialog);
            press(&mut app, KeyCode::Esc);
            assert_eq!(app.mode, AppMode::CardDetail);
        });
    }

    #[test]
    fn export_into_a_vanished_folder_keeps_the_dialog_open() {
        with_temp_dir(|| {
            let out = tempfile::tempdir().unwrap();
            let mut app = app_with_card("Card");
            app.export_dir = Some(out.path().to_path_buf());
            press(&mut app, KeyCode::Char('p'));
            std::fs::remove_dir_all(out.path()).unwrap();
            handle_input(&mut app, KeyEvent::new(KeyCode::Char('x'), KeyModifiers::empty())).unwrap();
            handle_input(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())).unwrap();
            assert_eq!(app.mode, AppMode::Dialog, "folder check keeps the dialog open");
        });
    }
}
