use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{App, AppMode, DialogKind, InsertTarget};
use crate::storage::board_store;

pub fn handle(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('j') | KeyCode::Down => {
            if !app.boards.is_empty() && app.selected_board_idx < app.boards.len() - 1 {
                app.selected_board_idx += 1;
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if app.selected_board_idx > 0 {
                app.selected_board_idx -= 1;
            }
        }
        KeyCode::Enter => {
            if let Some(board) = app.boards.get(app.selected_board_idx) {
                let id = board.id.clone();
                app.load_board(&id)?;
            }
        }
        KeyCode::Char('n') => {
            app.start_insert(InsertTarget::NewBoardName);
        }
        KeyCode::Char('c') => {
            if let Some(board) = app.boards.get_mut(app.selected_board_idx) {
                board.accent_color = board.accent_color.next();
                board_store::save_board(board)?;
                app.set_status("Board color changed".into());
            }
        }
        KeyCode::Char('d') => {
            if !app.boards.is_empty() {
                app.mode = AppMode::Dialog(DialogKind::ConfirmDeleteBoard);
            }
        }
        _ => {}
    }
    Ok(())
}
