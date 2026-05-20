use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, AppMode, DialogKind, InsertTarget};
use crate::storage::board_store;

pub fn handle(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);

    match (key.code, shift) {
        (KeyCode::Char('q'), _) => app.should_quit = true,
        (KeyCode::Down, false)
            if !app.boards.is_empty() && app.selected_board_idx < app.boards.len() - 1 => {
                app.selected_board_idx += 1;
            }
        (KeyCode::Up, false)
            if app.selected_board_idx > 0 => {
                app.selected_board_idx -= 1;
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
        (KeyCode::Char('r'), _) => {
            if let Some(board) = app.boards.get(app.selected_board_idx) {
                let name = board.name.clone();
                app.start_insert_with(InsertTarget::RenameBoard, &name);
            }
        }
        (KeyCode::Char('?'), _) => {
            app.previous_mode = Some(app.mode.clone());
            app.mode = AppMode::Help;
        }
        (KeyCode::Char('c'), _) => {
            if let Some(board) = app.boards.get_mut(app.selected_board_idx) {
                board.accent_color = board.accent_color.next();
                board_store::save_board(board)?;
                app.set_status("Board color changed".into());
            }
        }
        (KeyCode::Char('a'), _)
            if !app.boards.is_empty() => {
                app.mode = AppMode::Dialog(DialogKind::ConfirmArchiveBoard);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::model::board::BoardMeta;
    use crate::storage::board_store;
    use crate::test_support::with_temp_dir;

    fn seed_boards(names: &[&str]) -> Vec<BoardMeta> {
        names
            .iter()
            .map(|n| {
                let m = board_store::create_board((*n).to_string()).unwrap();
                board_store::append_to_order(&m.id).unwrap();
                m
            })
            .collect()
    }

    fn press(app: &mut App, code: KeyCode) {
        handle(app, KeyEvent::new(code, KeyModifiers::empty())).unwrap();
    }

    fn press_shift(app: &mut App, code: KeyCode) {
        handle(app, KeyEvent::new(code, KeyModifiers::SHIFT)).unwrap();
    }

    #[test]
    fn down_arrow_advances_selection_within_bounds() {
        with_temp_dir(|| {
            seed_boards(&["A", "B", "C"]);
            let mut app = App::new(None).unwrap();
            assert_eq!(app.boards.len(), 3);
            assert_eq!(app.selected_board_idx, 0);
            press(&mut app, KeyCode::Down);
            assert_eq!(app.selected_board_idx, 1);
            press(&mut app, KeyCode::Down);
            assert_eq!(app.selected_board_idx, 2);
            // Already at last — clamp
            press(&mut app, KeyCode::Down);
            assert_eq!(app.selected_board_idx, 2);
        });
    }

    #[test]
    fn up_arrow_stops_at_zero() {
        with_temp_dir(|| {
            seed_boards(&["A", "B"]);
            let mut app = App::new(None).unwrap();
            app.selected_board_idx = 1;
            press(&mut app, KeyCode::Up);
            assert_eq!(app.selected_board_idx, 0);
            press(&mut app, KeyCode::Up);
            assert_eq!(app.selected_board_idx, 0);
        });
    }

    #[test]
    fn shift_down_swaps_with_next_board_and_persists_order() {
        with_temp_dir(|| {
            let boards = seed_boards(&["A", "B", "C"]);
            let mut app = App::new(None).unwrap();
            assert_eq!(app.boards[0].name, "A");
            press_shift(&mut app, KeyCode::Down);
            assert_eq!(app.boards[0].name, "B");
            assert_eq!(app.boards[1].name, "A");
            assert_eq!(app.selected_board_idx, 1);
            // Order on disk must reflect the swap.
            let order = board_store::load_board_order().unwrap();
            assert_eq!(order[0], boards[1].id);
            assert_eq!(order[1], boards[0].id);
        });
    }

    #[test]
    fn shift_up_on_first_board_is_noop() {
        with_temp_dir(|| {
            seed_boards(&["A", "B"]);
            let mut app = App::new(None).unwrap();
            press_shift(&mut app, KeyCode::Up);
            assert_eq!(app.boards[0].name, "A");
            assert_eq!(app.selected_board_idx, 0);
        });
    }

    #[test]
    fn n_enters_new_board_name_insert_mode() {
        with_temp_dir(|| {
            let mut app = App::new(None).unwrap();
            press(&mut app, KeyCode::Char('n'));
            assert!(matches!(
                app.mode,
                AppMode::Insert(InsertTarget::NewBoardName)
            ));
            assert!(app.input_buffer.is_empty());
        });
    }

    #[test]
    fn r_prefills_rename_with_existing_name() {
        with_temp_dir(|| {
            seed_boards(&["My Board"]);
            let mut app = App::new(None).unwrap();
            press(&mut app, KeyCode::Char('r'));
            assert!(matches!(
                app.mode,
                AppMode::Insert(InsertTarget::RenameBoard)
            ));
            assert_eq!(app.input_buffer, "My Board");
        });
    }

    #[test]
    fn c_cycles_board_accent_color_and_persists() {
        with_temp_dir(|| {
            seed_boards(&["A"]);
            let mut app = App::new(None).unwrap();
            let before = app.boards[0].accent_color;
            press(&mut app, KeyCode::Char('c'));
            let after = app.boards[0].accent_color;
            assert_ne!(before, after);
            // Reload from disk and verify persistence.
            let reloaded = board_store::load_board(&app.boards[0].id).unwrap();
            assert_eq!(reloaded.accent_color, after);
        });
    }

    #[test]
    fn a_opens_archive_confirmation_when_boards_exist() {
        with_temp_dir(|| {
            seed_boards(&["A"]);
            let mut app = App::new(None).unwrap();
            press(&mut app, KeyCode::Char('a'));
            assert!(matches!(
                app.mode,
                AppMode::Dialog(DialogKind::ConfirmArchiveBoard)
            ));
        });
    }

    #[test]
    fn a_is_noop_when_no_boards() {
        with_temp_dir(|| {
            let mut app = App::new(None).unwrap();
            assert!(app.boards.is_empty());
            press(&mut app, KeyCode::Char('a'));
            // Stay in BoardSelector — no dialog opens.
            assert!(matches!(app.mode, AppMode::BoardSelector));
        });
    }

    #[test]
    fn v_opens_archived_boards_dialog() {
        with_temp_dir(|| {
            let mut app = App::new(None).unwrap();
            press(&mut app, KeyCode::Char('v'));
            assert!(matches!(
                app.mode,
                AppMode::Dialog(DialogKind::ArchivedBoards)
            ));
        });
    }

    #[test]
    fn question_mark_opens_help_and_remembers_previous_mode() {
        with_temp_dir(|| {
            let mut app = App::new(None).unwrap();
            press(&mut app, KeyCode::Char('?'));
            assert!(matches!(app.mode, AppMode::Help));
            assert!(matches!(
                app.previous_mode.as_ref().unwrap(),
                AppMode::BoardSelector
            ));
        });
    }

    #[test]
    fn q_sets_should_quit() {
        with_temp_dir(|| {
            let mut app = App::new(None).unwrap();
            press(&mut app, KeyCode::Char('q'));
            assert!(app.should_quit);
        });
    }
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
