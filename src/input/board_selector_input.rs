use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, AppMode, DialogKind, InsertTarget};
use crate::storage::board_store;

pub fn handle(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);

    match (key.code, shift) {
        (KeyCode::Char('q'), _) => app.should_quit = true,
        (KeyCode::Down, false) => {
            if !app.boards.is_empty() && app.selected_board_idx < app.boards.len() - 1 {
                app.selected_board_idx += 1;
            }
        }
        (KeyCode::Up, false) => {
            if app.selected_board_idx > 0 {
                app.selected_board_idx -= 1;
            }
        }
        (KeyCode::Down, true) => {
            move_board(app, 1)?;
        }
        (KeyCode::Up, true) => {
            move_board(app, -1)?;
        }
        (KeyCode::Enter, _) => {
            if let Some(board) = app.boards.get(app.selected_board_idx) {
                let id = board.id.clone();
                app.load_board(&id)?;
            }
        }
        (KeyCode::Char('n'), _) => {
            app.start_insert(InsertTarget::NewBoardName);
        }
        (KeyCode::Char('c'), _) => {
            if let Some(board) = app.boards.get_mut(app.selected_board_idx) {
                board.accent_color = board.accent_color.next();
                board_store::save_board(board)?;
                app.set_status("Board color changed".into());
            }
        }
        (KeyCode::Char('d'), _) => {
            if !app.boards.is_empty() {
                app.mode = AppMode::Dialog(DialogKind::ConfirmArchiveBoard);
            }
        }
        (KeyCode::Char('v'), _) => {
            let archived = board_store::list_archived_boards()?;
            app.archived_boards = archived;
            app.archived_selected = 0;
            app.mode = AppMode::Dialog(DialogKind::ArchivedBoards);
        }
        _ => {}
    }
    Ok(())
}

fn move_board(app: &mut App, direction: i32) -> anyhow::Result<()> {
    if app.boards.is_empty() {
        return Ok(());
    }
    let idx = app.selected_board_idx;
    let new_idx = idx as i32 + direction;
    if new_idx < 0 || new_idx >= app.boards.len() as i32 {
        return Ok(());
    }
    let new_idx = new_idx as usize;

    let mut order = board_store::load_board_order().unwrap_or_default();

    // Ensure both boards are in the order list
    let a_id = app.boards[idx].id.clone();
    let b_id = app.boards[new_idx].id.clone();

    // Build order from current display if order is missing entries
    if !order.contains(&a_id) || !order.contains(&b_id) {
        let all_ids: Vec<_> = app.boards.iter().map(|b| b.id.clone()).collect();
        for id in &all_ids {
            if !order.contains(id) {
                order.push(id.clone());
            }
        }
    }

    // Swap positions in the order vec
    if let (Some(pos_a), Some(pos_b)) = (
        order.iter().position(|id| id == &a_id),
        order.iter().position(|id| id == &b_id),
    ) {
        order.swap(pos_a, pos_b);
        board_store::save_board_order(&order)?;
        app.boards.swap(idx, new_idx);
        app.selected_board_idx = new_idx;
    }

    Ok(())
}
