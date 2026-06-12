use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, AppMode};
use crate::board_directory;
use crate::dialog::{archived_boards::ArchivedBoards, confirm_archive_board::ConfirmArchiveBoard};
use crate::insert::line_editor;

use super::keymap::{self, Binding};

#[derive(Clone, Copy)]
pub enum Action {
    SelectUp,
    SelectDown,
    MoveBoardUp,
    MoveBoardDown,
    OpenBoard,
    NewBoard,
    RenameBoard,
    CycleAccentColor,
    ArchiveBoard,
    ViewArchived,
    Help,
    Quit,
}

/// Board-selector keymap. Single definition of key → action → help text;
/// the help overlay renders from this table.
pub static KEYMAP: &[Binding<Action>] = &[
    // Navigation
    Binding { code: KeyCode::Up, shift: Some(false), action: Action::SelectUp, keys: "Up / Down", help: "Navigate boards", section: "Navigation" },
    Binding { code: KeyCode::Down, shift: Some(false), action: Action::SelectDown, keys: "Up / Down", help: "Navigate boards", section: "Navigation" },
    Binding { code: KeyCode::Up, shift: Some(true), action: Action::MoveBoardUp, keys: "Shift+Up/Down", help: "Reorder board", section: "Navigation" },
    Binding { code: KeyCode::Down, shift: Some(true), action: Action::MoveBoardDown, keys: "Shift+Up/Down", help: "Reorder board", section: "Navigation" },
    Binding { code: KeyCode::Enter, shift: None, action: Action::OpenBoard, keys: "Enter", help: "Open board", section: "Navigation" },
    // Actions
    Binding { code: KeyCode::Char('n'), shift: None, action: Action::NewBoard, keys: "n", help: "New board", section: "Actions" },
    Binding { code: KeyCode::Char('r'), shift: None, action: Action::RenameBoard, keys: "r", help: "Rename board", section: "Actions" },
    Binding { code: KeyCode::Char('c'), shift: None, action: Action::CycleAccentColor, keys: "c", help: "Cycle accent color", section: "Actions" },
    Binding { code: KeyCode::Char('a'), shift: None, action: Action::ArchiveBoard, keys: "a", help: "Archive board", section: "Actions" },
    Binding { code: KeyCode::Char('v'), shift: None, action: Action::ViewArchived, keys: "v", help: "View archived", section: "Actions" },
    // App
    Binding { code: KeyCode::Char('?'), shift: None, action: Action::Help, keys: "?", help: "Help", section: "App" },
    Binding { code: KeyCode::Char('q'), shift: None, action: Action::Quit, keys: "q", help: "Quit", section: "App" },
];

pub fn handle(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    let Some(action) = keymap::lookup(KEYMAP, key.code, shift) else {
        return Ok(());
    };
    run(app, action)
}

fn run(app: &mut App, action: Action) -> anyhow::Result<()> {
    match action {
        Action::Quit => app.should_quit = true,
        Action::SelectDown => {
            if !app.boards.is_empty() && app.selected_board_idx < app.boards.len() - 1 {
                app.selected_board_idx += 1;
            }
        }
        Action::SelectUp => {
            if app.selected_board_idx > 0 {
                app.selected_board_idx -= 1;
            }
        }
        Action::MoveBoardDown => {
            move_board(app, 1)?;
        }
        Action::MoveBoardUp => {
            move_board(app, -1)?;
        }
        Action::OpenBoard => {
            if let Some(board) = app.boards.get(app.selected_board_idx) {
                let id = board.id.clone();
                app.load_board(&id)?;
            }
        }
        Action::NewBoard => {
            app.start_insert(Box::new(line_editor::NewBoardName::new()));
        }
        Action::RenameBoard => {
            if let Some(board) = app.boards.get(app.selected_board_idx) {
                let name = board.name.clone();
                app.start_insert(Box::new(line_editor::RenameBoard::new(&name)));
            }
        }
        Action::Help => {
            app.previous_mode = Some(app.mode.clone());
            app.mode = AppMode::Help;
        }
        Action::CycleAccentColor => {
            if let Some(board) = app.boards.get(app.selected_board_idx) {
                let id = board.id.clone();
                let next = board_directory::cycle_accent(&id)?;
                if let Some(b) = app.boards.get_mut(app.selected_board_idx) {
                    b.accent_color = next;
                }
                app.set_status("Board color changed".into());
            }
        }
        Action::ArchiveBoard => {
            if !app.boards.is_empty() {
                let board_name = app
                    .boards
                    .get(app.selected_board_idx)
                    .map(|b| b.name.clone())
                    .unwrap_or_default();
                app.open_dialog(Box::new(ConfirmArchiveBoard { board_name }));
            }
        }
        Action::ViewArchived => {
            let archived = board_directory::list_archived()?;
            app.open_dialog(Box::new(ArchivedBoards {
                boards: archived,
                selected: 0,
            }));
        }
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
            assert!(matches!(app.mode, AppMode::Insert));
            let h = app.insert.as_ref().unwrap();
            assert_eq!(h.line_buffer(), Some(""));
        });
    }

    #[test]
    fn r_prefills_rename_with_existing_name() {
        with_temp_dir(|| {
            seed_boards(&["My Board"]);
            let mut app = App::new(None).unwrap();
            press(&mut app, KeyCode::Char('r'));
            assert!(matches!(app.mode, AppMode::Insert));
            let h = app.insert.as_ref().unwrap();
            assert_eq!(h.line_buffer(), Some("My Board"));
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
            assert!(matches!(app.mode, AppMode::Dialog));
            assert!(app.dialog.is_some());
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
            assert!(matches!(app.mode, AppMode::Dialog));
            assert!(app.dialog.is_some());
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

    let a_id = app.boards[idx].id.clone();
    let b_id = app.boards[new_idx].id.clone();
    let displayed: Vec<_> = app.boards.iter().map(|b| b.id.clone()).collect();
    board_directory::swap_order(&a_id, &b_id, &displayed)?;
    app.boards.swap(idx, new_idx);
    app.selected_board_idx = new_idx;

    Ok(())
}
