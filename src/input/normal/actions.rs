use crate::app::{App, AppMode};
use crate::command::{Command, MoveDir};
use crate::dialog::{
    archived_cards::ArchivedCards, archived_lists::ArchivedLists, card_history::CardHistory,
    confirm_archive_card::ConfirmArchiveCard, confirm_archive_list::ConfirmArchiveList,
    confirm_clear_due_date::ConfirmClearDueDate, label_manager::LabelManager,
    label_picker::LabelPicker,
};
use crate::insert::{
    InsertSurface, date_picker::DatePicker, line_editor, markdown_editor::MarkdownEditor,
};

use super::*;

/// `C`: commit the data directory now (works whether or not commit-on-quit
/// is enabled).
pub(super) fn commit_to_git(app: &mut App) {
    use crate::git_sync::{self, Outcome};
    let msg = match git_sync::commit() {
        Ok(Outcome::Committed(subject)) => format!("Committed: {subject}"),
        Ok(Outcome::Nothing) => "Nothing to commit".to_string(),
        Err(e) => e.to_string(),
    };
    app.set_status(msg);
}

/// Switch the open board to its neighbor in display order, wrapping around.
/// No-op with fewer than two boards.
pub(super) fn cycle_open_board(app: &mut App, delta: i32) -> anyhow::Result<()> {
    let len = app.boards.len();
    if len < 2 {
        return Ok(());
    }
    let Some(current_id) = app.board().map(|b| b.meta.id.clone()) else {
        return Ok(());
    };
    let Some(idx) = app.boards.iter().position(|b| b.id == current_id) else {
        return Ok(());
    };
    let new_idx = (idx as i32 + delta).rem_euclid(len as i32) as usize;
    if new_idx == idx {
        return Ok(());
    }
    let new_id = app.boards[new_idx].id.clone();
    app.load_board(&new_id)?;
    app.selected_board_idx = new_idx;
    app.set_status(format!("Switched to board '{}'", app.boards[new_idx].name));
    Ok(())
}

pub(super) fn run(app: &mut App, action: Action) -> anyhow::Result<()> {
    match action {
        Action::Quit => app.should_quit = true,
        Action::Help => {
            app.remember_return_mode();
            app.mode = AppMode::Help;
        }
        Action::CloseBoard => {
            app.close_board()?;
        }
        Action::SelectListLeft => {
            if let Some(editor) = &mut app.editor {
                editor.select_list_left();
            }
        }
        Action::SelectListRight => {
            if let Some(editor) = &mut app.editor {
                editor.select_list_right();
            }
        }
        Action::SelectCardDown => {
            let search = app.search_active.then_some(app.search_query.as_str());
            if let Some(editor) = &mut app.editor {
                editor.select_card_down(search);
            }
        }
        Action::SelectCardUp => {
            let search = app.search_active.then_some(app.search_query.as_str());
            if let Some(editor) = &mut app.editor {
                editor.select_card_up(search);
            }
        }
        Action::MoveCardLeft => move_card_in_direction(app, MoveDir::Left)?,
        Action::MoveCardRight => move_card_in_direction(app, MoveDir::Right)?,
        Action::MoveCardUp => move_card_in_direction(app, MoveDir::Up)?,
        Action::MoveCardDown => move_card_in_direction(app, MoveDir::Down)?,
        Action::GrabCard => {
            if app.board().and_then(|b| b.current_card_id()).is_some() {
                app.grab_active = true;
            }
        }
        Action::JumpFirstCard => {
            if let Some(editor) = &mut app.editor {
                editor.select_first_card();
            }
        }
        Action::JumpLastCard => {
            if let Some(editor) = &mut app.editor {
                editor.select_last_card();
            }
        }
        Action::SwitchBoardPrev => cycle_open_board(app, -1)?,
        Action::SwitchBoardNext => cycle_open_board(app, 1)?,
        Action::OpenCardDetail => {
            if let Some(editor) = &mut app.editor
                && editor.board().current_card_id().is_some()
            {
                editor.reset_detail_cursor();
                app.mode = AppMode::CardDetail;
            }
        }
        Action::CopyTitle => {
            if let Some(board) = app.board()
                && let Some(card) = board.current_card()
            {
                let title = card.title.clone();
                let what = format!("title '{title}'");
                app.copy_to_clipboard(title, &what);
            }
        }
        Action::EditDescription => {
            if let Some(board) = app.board()
                && let Some(card) = board.current_card()
            {
                let desc = card.description.clone();
                let card_id = card.id.clone();
                app.start_insert(Box::new(MarkdownEditor::new(card_id, &desc)));
            }
        }
        Action::EditTitle => {
            if let Some(board) = app.board()
                && let Some(card) = board.current_card()
            {
                let title = card.title.clone();
                let card_id = card.id.clone();
                app.start_insert(Box::new(line_editor::EditCardTitle::new(
                    card_id, &title, true,
                )));
            }
        }
        Action::NewCard => {
            if app.editor.is_some() {
                app.start_insert(Box::new(line_editor::NewCardTitle::new()));
            }
        }
        Action::NewList => {
            if app.editor.is_some() {
                app.start_insert(Box::new(line_editor::NewListName::new()));
            }
        }
        Action::RenameList => {
            if let Some(board) = app.board()
                && let Some(list) = board.lists.get(board.selected_list)
            {
                let name = list.name.clone();
                let list_id = list.id.clone();
                app.start_insert(Box::new(line_editor::RenameList::new(list_id, &name)));
            }
        }
        Action::ArchiveList => {
            if let Some(board) = app.board()
                && !board.lists.is_empty()
            {
                app.open_dialog(Box::new(ConfirmArchiveList));
            }
        }
        Action::MoveListLeft => move_list_in_direction(app, MoveDir::Left)?,
        Action::MoveListRight => move_list_in_direction(app, MoveDir::Right)?,
        Action::ArchiveCard => {
            if let Some(board) = app.board()
                && board.current_card_id().is_some()
            {
                app.open_dialog(Box::new(ConfirmArchiveCard));
            }
        }
        Action::ViewArchivedCards => {
            if let Some(editor) = &app.editor {
                match editor.archived_cards() {
                    Ok(archived) if archived.is_empty() => {
                        app.set_status("No archived cards".into());
                    }
                    Ok(archived) => {
                        app.open_dialog(Box::new(ArchivedCards {
                            cards: archived,
                            selected: 0,
                        }));
                    }
                    Err(e) => app.set_status(format!("Error: {e}")),
                }
            }
        }
        Action::ViewArchivedLists => {
            if let Some(editor) = &app.editor {
                let archived = editor.archived_lists();
                if archived.is_empty() {
                    app.set_status("No archived lists".into());
                } else {
                    app.open_dialog(Box::new(ArchivedLists {
                        lists: archived,
                        selected: 0,
                    }));
                }
            }
        }
        Action::Search => {
            app.search_query.clear();
            app.mode = AppMode::Command;
        }
        Action::FilterByLabel => {
            app.open_dialog(Box::new(LabelPicker { selected_idx: 0 }));
        }
        Action::ClearFilters => {
            app.search_active = false;
            app.search_query.clear();
            app.label_filter = None;
            app.set_status("Filters cleared".into());
        }
        Action::AssignLabels => {
            if let Some(board) = app.board()
                && board.current_card_id().is_some()
            {
                app.open_dialog(Box::new(LabelPicker { selected_idx: 0 }));
            }
        }
        Action::ManageLabels => {
            app.open_dialog(Box::new(LabelManager {
                selected_idx: 0,
                from_picker: false,
            }));
        }
        Action::SetDueDate => {
            if let Some(board) = app.board()
                && let Some(card) = board.current_card()
            {
                let date_str = card
                    .due_date
                    .map(|d| d.format("%Y-%m-%d").to_string())
                    .unwrap_or_default();
                let card_id = card.id.clone();
                app.start_insert(Box::new(DatePicker::new(
                    card_id,
                    &date_str,
                    InsertSurface::BoardView,
                )));
            }
        }
        Action::CardHistoryDialog => {
            if let Some(board) = app.board()
                && board.current_card_id().is_some()
            {
                app.open_dialog(Box::new(CardHistory::new()));
            }
        }
        Action::ExportPdf => app.open_export_dialog(crate::export::Theme::Mono),
        Action::ExportPdfColor => app.open_export_dialog(crate::export::Theme::Color),
        Action::Commit => commit_to_git(app),
        Action::ToggleCreatedDate => toggle_created_date(app)?,
        Action::ToggleUpdatedDate => toggle_updated_date(app)?,
        Action::ClearDueDate => {
            if let Some(board) = app.board()
                && board.current_card().is_some_and(|c| c.due_date.is_some())
            {
                app.open_dialog(Box::new(ConfirmClearDueDate));
            }
        }
    }
    Ok(())
}

/// Shared with the card-detail keymap: both views show the board's setting.
pub(in crate::input) fn toggle_created_date(app: &mut App) -> anyhow::Result<()> {
    if let Some(board) = app.board() {
        let show = !board.meta.show_created_date;
        app.apply(Command::SetShowCreatedDate { show })?;
        app.set_status(format!(
            "Created dates {}",
            if show { "shown" } else { "hidden" }
        ));
    }
    Ok(())
}

pub(in crate::input) fn toggle_updated_date(app: &mut App) -> anyhow::Result<()> {
    if let Some(board) = app.board() {
        let show = !board.meta.show_updated_date;
        app.apply(Command::SetShowUpdatedDate { show })?;
        app.set_status(format!(
            "Updated dates {}",
            if show { "shown" } else { "hidden" }
        ));
    }
    Ok(())
}

pub(super) fn move_list_in_direction(app: &mut App, direction: MoveDir) -> anyhow::Result<()> {
    let list_id = app
        .board()
        .and_then(|b| b.lists.get(b.selected_list).map(|l| l.id.clone()));
    if let Some(list_id) = list_id {
        app.apply(Command::MoveList { list_id, direction })?;
    }
    Ok(())
}

pub(super) fn move_card_in_direction(app: &mut App, direction: MoveDir) -> anyhow::Result<()> {
    let card_id = match app.board().and_then(|b| b.current_card_id().cloned()) {
        Some(id) => id,
        None => return Ok(()),
    };
    app.apply(Command::MoveCard { card_id, direction })?;
    Ok(())
}
