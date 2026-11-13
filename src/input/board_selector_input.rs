use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, AppMode};
use crate::board_directory;
use crate::dialog::{
    archived_boards::ArchivedBoards, color_picker::ColorPicker,
    confirm_archive_board::ConfirmArchiveBoard,
};
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
    BoardConfiguration,
    ArchiveBoard,
    ViewArchived,
    Help,
    Quit,
}

/// Board-selector keymap. Single definition of key → action → help text;
/// the help overlay renders from this table.
pub static KEYMAP: &[Binding<Action>] = &[
    // Navigation
    Binding {
        code: KeyCode::Up,
        shift: Some(false),
        action: Action::SelectUp,
        keys: "↑ / ↓",
        help: "select board",
        section: "Navigation",
    },
    Binding {
        code: KeyCode::Down,
        shift: Some(false),
        action: Action::SelectDown,
        keys: "↑ / ↓",
        help: "select board",
        section: "Navigation",
    },
    Binding {
        code: KeyCode::Up,
        shift: Some(true),
        action: Action::MoveBoardUp,
        keys: "shift+↑/↓",
        help: "reorder board",
        section: "Navigation",
    },
    Binding {
        code: KeyCode::Down,
        shift: Some(true),
        action: Action::MoveBoardDown,
        keys: "shift+↑/↓",
        help: "reorder board",
        section: "Navigation",
    },
    Binding {
        code: KeyCode::Enter,
        shift: None,
        action: Action::OpenBoard,
        keys: "enter",
        help: "open board",
        section: "Navigation",
    },
    // Actions
    Binding {
        code: KeyCode::Char('n'),
        shift: None,
        action: Action::NewBoard,
        keys: "n",
        help: "new board",
        section: "Actions",
    },
    Binding {
        code: KeyCode::Char('r'),
        shift: None,
        action: Action::RenameBoard,
        keys: "r",
        help: "rename board",
        section: "Actions",
    },
    Binding {
        code: KeyCode::Char('c'),
        shift: None,
        action: Action::CycleAccentColor,
        keys: "c",
        help: "cycle accent color",
        section: "Actions",
    },
    Binding {
        code: KeyCode::Char('C'),
        shift: None,
        action: Action::BoardConfiguration,
        keys: "C",
        help: "board configuration",
        section: "Actions",
    },
    Binding {
        code: KeyCode::Char('a'),
        shift: None,
        action: Action::ArchiveBoard,
        keys: "a",
        help: "archive board",
        section: "Actions",
    },
    Binding {
        code: KeyCode::Char('v'),
        shift: None,
        action: Action::ViewArchived,
        keys: "v",
        help: "view archived boards",
        section: "Actions",
    },
    // App
    Binding {
        code: KeyCode::Char('?'),
        shift: None,
        action: Action::Help,
        keys: "?",
        help: "",
        section: "App",
    },
    Binding {
        code: KeyCode::Char('q'),
        shift: None,
        action: Action::Quit,
        keys: "q",
        help: "quit",
        section: "App",
    },
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
            app.remember_return_mode();
            app.mode = AppMode::Help;
        }
        Action::CycleAccentColor => {
            if let Some(board) = app.boards.get(app.selected_board_idx) {
                let id = board.id.clone();
                let next = board_directory::cycle_accent(&id)?;
                if let Some(b) = app.boards.get_mut(app.selected_board_idx) {
                    b.accent_color = next;
                }
                app.set_status(format!("Board color set to {}", next.display_name()));
            }
        }
        Action::BoardConfiguration => {
            if let Some(board) = app.boards.get(app.selected_board_idx) {
                let color = board.accent_color;
                let new_card_position = board.new_card_position;
                app.open_dialog(Box::new(ColorPicker::for_board(color, new_card_position)));
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

    board_directory::reorder(&app.boards, idx, new_idx)?;
    app.boards.swap(idx, new_idx);
    app.selected_board_idx = new_idx;

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
            .enumerate()
            .map(|(idx, n)| {
                let mut m = board_store::create_board((*n).to_string()).unwrap();
                m.position = (idx + 1) as f64;
                board_store::save_board(&m).unwrap();
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
            let stored = crate::board_directory::list().unwrap();
            assert_eq!(stored[0].id, boards[1].id);
            assert_eq!(stored[1].id, boards[0].id);
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
    fn shift_c_opens_board_configuration_dialog() {
        with_temp_dir(|| {
            seed_boards(&["A"]);
            let mut app = App::new(None).unwrap();
            press(&mut app, KeyCode::Char('C'));
            assert!(matches!(app.mode, AppMode::Dialog));
            assert!(app.dialog.is_some());
        });
    }

    #[test]
    fn board_configuration_dialog_configures_new_card_position_top() {
        with_temp_dir(|| {
            let _ = seed_boards(&["Work"]);
            let mut app = App::new(None).unwrap();

            let send = |app: &mut App, code: KeyCode| {
                crate::input::handle_input(app, KeyEvent::new(code, KeyModifiers::empty()))
                    .unwrap();
            };

            // Press C to open Board Configuration
            send(&mut app, KeyCode::Char('C'));
            assert!(matches!(app.mode, AppMode::Dialog));

            // Navigate down to field 3 ("New items")
            send(&mut app, KeyCode::Down);
            send(&mut app, KeyCode::Down);
            send(&mut app, KeyCode::Down);

            // Press Left to set to Top
            send(&mut app, KeyCode::Left);

            // Press Enter to confirm and save
            send(&mut app, KeyCode::Enter);
            assert!(matches!(app.mode, AppMode::BoardSelector));
            assert_eq!(
                app.status_message.as_ref().map(|(m, _)| m.as_str()),
                Some("Board configuration saved")
            );
            assert_eq!(
                app.boards[0].new_card_position,
                crate::model::board::NewCardPosition::Top
            );

            // Open the board
            send(&mut app, KeyCode::Enter);
            assert!(matches!(app.mode, AppMode::Normal));

            // Verify loaded board has Top
            assert_eq!(
                app.board().unwrap().meta.new_card_position,
                crate::model::board::NewCardPosition::Top
            );

            // Add a list
            app.apply(crate::command::Command::AddList {
                name: "Todo".into(),
            })
            .unwrap();
            let list_id = app.board().unwrap().lists[0].id.clone();

            // Add first card
            app.apply(crate::command::Command::AddCard {
                list_id: list_id.clone(),
                title: "first".into(),
            })
            .unwrap();

            // Add second card
            app.apply(crate::command::Command::AddCard {
                list_id,
                title: "second".into(),
            })
            .unwrap();

            // "second" should be at index 0, "first" at index 1
            let board = app.board().unwrap();
            let card_ids = &board.lists[0].card_ids;
            assert_eq!(board.cards[&card_ids[0]].title, "second");
            assert_eq!(board.cards[&card_ids[1]].title, "first");
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
    fn unarchive_board_requires_dialog_confirmation() {
        with_temp_dir(|| {
            let boards = seed_boards(&["Board to Archive"]);
            let board_id = boards[0].id.clone();
            let mut app = App::new(None).unwrap();

            // Archive the board: 'a' opens ConfirmArchiveBoard, 'y' confirms
            press(&mut app, KeyCode::Char('a'));
            assert!(matches!(app.mode, AppMode::Dialog));
            crate::input::handle_input(
                &mut app,
                KeyEvent::new(KeyCode::Char('y'), KeyModifiers::empty()),
            )
            .unwrap();
            assert_eq!(app.mode, AppMode::BoardSelector);
            assert!(app.boards.is_empty());

            // Open archived boards dialog
            press(&mut app, KeyCode::Char('v'));
            assert_eq!(app.mode, AppMode::Dialog);

            // Press Enter to restore: opens ConfirmRestoreBoard dialog
            crate::input::handle_input(
                &mut app,
                KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
            )
            .unwrap();
            assert_eq!(app.mode, AppMode::Dialog);

            // Cancel with 'n'
            crate::input::handle_input(
                &mut app,
                KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty()),
            )
            .unwrap();
            assert_eq!(app.mode, AppMode::Dialog);
            assert!(app.boards.is_empty());

            // Press Enter then 'y' to confirm restore
            crate::input::handle_input(
                &mut app,
                KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
            )
            .unwrap();
            crate::input::handle_input(
                &mut app,
                KeyEvent::new(KeyCode::Char('y'), KeyModifiers::empty()),
            )
            .unwrap();
            assert_eq!(app.mode, AppMode::BoardSelector);
            assert_eq!(app.boards.len(), 1);
            assert_eq!(app.boards[0].id, board_id);
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
