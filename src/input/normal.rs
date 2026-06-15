use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, AppMode};
use crate::command::{Command, MoveDir};
use crate::dialog::{
    archived_cards::ArchivedCards, archived_lists::ArchivedLists, card_history::CardHistory,
    confirm_archive_card::ConfirmArchiveCard, confirm_archive_list::ConfirmArchiveList,
    label_manager::LabelManager, label_picker::LabelPicker,
};
use crate::insert::{
    date_picker::DatePicker, line_editor, markdown_editor::MarkdownEditor, InsertSurface,
};

use super::keymap::{self, Binding};

#[derive(Clone, Copy)]
pub enum Action {
    Quit,
    Help,
    CloseBoard,
    SelectListLeft,
    SelectListRight,
    SelectCardUp,
    SelectCardDown,
    MoveCardLeft,
    MoveCardRight,
    MoveCardUp,
    MoveCardDown,
    JumpFirstCard,
    JumpLastCard,
    SwitchBoardPrev,
    SwitchBoardNext,
    OpenCardDetail,
    CopyTitle,
    EditDescription,
    EditTitle,
    NewCard,
    ArchiveCard,
    ViewArchivedCards,
    CardHistoryDialog,
    NewList,
    RenameList,
    ArchiveList,
    ViewArchivedLists,
    MoveListLeft,
    MoveListRight,
    Search,
    FilterByLabel,
    ClearFilters,
    AssignLabels,
    ManageLabels,
    SetDueDate,
    ClearDueDate,
}

/// Board-view keymap. Single definition of key → action → help text; the
/// help overlay renders from this table.
pub static KEYMAP: &[Binding<Action>] = &[
    // Navigation
    Binding { code: KeyCode::Left, shift: Some(false), action: Action::SelectListLeft, keys: "Left / Right", help: "Switch lists", section: "Navigation" },
    Binding { code: KeyCode::Right, shift: Some(false), action: Action::SelectListRight, keys: "Left / Right", help: "Switch lists", section: "Navigation" },
    Binding { code: KeyCode::Up, shift: Some(false), action: Action::SelectCardUp, keys: "Up / Down", help: "Navigate cards", section: "Navigation" },
    Binding { code: KeyCode::Down, shift: Some(false), action: Action::SelectCardDown, keys: "Up / Down", help: "Navigate cards", section: "Navigation" },
    Binding { code: KeyCode::Char('g'), shift: None, action: Action::JumpFirstCard, keys: "g / G", help: "First / last card", section: "Navigation" },
    Binding { code: KeyCode::Char('G'), shift: None, action: Action::JumpLastCard, keys: "g / G", help: "First / last card", section: "Navigation" },
    Binding { code: KeyCode::Char('j'), shift: None, action: Action::SwitchBoardPrev, keys: "j / k", help: "Prev / next board", section: "Navigation" },
    Binding { code: KeyCode::Char('k'), shift: None, action: Action::SwitchBoardNext, keys: "j / k", help: "Prev / next board", section: "Navigation" },
    Binding { code: KeyCode::Enter, shift: None, action: Action::OpenCardDetail, keys: "Enter", help: "Open card detail", section: "Navigation" },
    // Card
    Binding { code: KeyCode::Char('t'), shift: None, action: Action::EditTitle, keys: "t", help: "Quick-edit title", section: "Card" },
    Binding { code: KeyCode::Char('e'), shift: None, action: Action::EditDescription, keys: "e", help: "Edit description", section: "Card" },
    Binding { code: KeyCode::Char('y'), shift: None, action: Action::CopyTitle, keys: "y", help: "Copy title", section: "Card" },
    Binding { code: KeyCode::Char('n'), shift: None, action: Action::NewCard, keys: "n", help: "New card", section: "Card" },
    Binding { code: KeyCode::Char('a'), shift: None, action: Action::ArchiveCard, keys: "a", help: "Archive card", section: "Card" },
    Binding { code: KeyCode::Char('v'), shift: None, action: Action::ViewArchivedCards, keys: "v", help: "View archived cards", section: "Card" },
    Binding { code: KeyCode::Char('h'), shift: None, action: Action::CardHistoryDialog, keys: "h", help: "View change history", section: "Card" },
    // Move
    Binding { code: KeyCode::Left, shift: Some(true), action: Action::MoveCardLeft, keys: "Shift+Left/Right", help: "Move to adjacent list", section: "Move" },
    Binding { code: KeyCode::Right, shift: Some(true), action: Action::MoveCardRight, keys: "Shift+Left/Right", help: "Move to adjacent list", section: "Move" },
    Binding { code: KeyCode::Up, shift: Some(true), action: Action::MoveCardUp, keys: "Shift+Up/Down", help: "Move within list", section: "Move" },
    Binding { code: KeyCode::Down, shift: Some(true), action: Action::MoveCardDown, keys: "Shift+Up/Down", help: "Move within list", section: "Move" },
    // Lists
    Binding { code: KeyCode::Char('N'), shift: None, action: Action::NewList, keys: "N", help: "New list", section: "Lists" },
    Binding { code: KeyCode::Char('r'), shift: None, action: Action::RenameList, keys: "r", help: "Rename list", section: "Lists" },
    Binding { code: KeyCode::Char('A'), shift: None, action: Action::ArchiveList, keys: "A", help: "Archive list", section: "Lists" },
    Binding { code: KeyCode::Char('V'), shift: None, action: Action::ViewArchivedLists, keys: "V", help: "View archived lists", section: "Lists" },
    Binding { code: KeyCode::Char('<'), shift: None, action: Action::MoveListLeft, keys: "< / >", help: "Reorder list", section: "Lists" },
    Binding { code: KeyCode::Char('>'), shift: None, action: Action::MoveListRight, keys: "< / >", help: "Reorder list", section: "Lists" },
    // Search & Filter
    Binding { code: KeyCode::Char('/'), shift: None, action: Action::Search, keys: "/", help: "Search", section: "Search & Filter" },
    Binding { code: KeyCode::Char('f'), shift: None, action: Action::FilterByLabel, keys: "f", help: "Filter by label", section: "Search & Filter" },
    Binding { code: KeyCode::Char('F'), shift: None, action: Action::ClearFilters, keys: "F", help: "Clear filters", section: "Search & Filter" },
    // Labels & Due
    Binding { code: KeyCode::Char('l'), shift: None, action: Action::AssignLabels, keys: "l", help: "Assign / remove labels", section: "Labels & Due" },
    Binding { code: KeyCode::Char('L'), shift: None, action: Action::ManageLabels, keys: "L", help: "Manage labels", section: "Labels & Due" },
    Binding { code: KeyCode::Char('u'), shift: None, action: Action::SetDueDate, keys: "u", help: "Set due date", section: "Labels & Due" },
    Binding { code: KeyCode::Char('U'), shift: None, action: Action::ClearDueDate, keys: "U", help: "Clear due date", section: "Labels & Due" },
    // App
    Binding { code: KeyCode::Char('b'), shift: None, action: Action::CloseBoard, keys: "b", help: "Back to selector", section: "App" },
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

/// Switch the open board to its neighbor in display order, wrapping around.
/// No-op with fewer than two boards.
fn cycle_open_board(app: &mut App, delta: i32) -> anyhow::Result<()> {
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
    app.set_status(format!("Switched to '{}'", app.boards[new_idx].name));
    Ok(())
}

fn run(app: &mut App, action: Action) -> anyhow::Result<()> {
    match action {
        Action::Quit => app.should_quit = true,
        Action::Help => {
            app.previous_mode = Some(app.mode.clone());
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
                && editor.board().current_card_id().is_some() {
                    editor.reset_detail_cursor();
                    app.mode = AppMode::CardDetail;
                }
        }
        Action::CopyTitle => {
            if let Some(board) = app.board()
                && let Some(card) = board.current_card() {
                    let title = card.title.clone();
                    app.copy_to_clipboard(title);
                }
        }
        Action::EditDescription => {
            if let Some(board) = app.board()
                && let Some(card) = board.current_card() {
                    let desc = card.description.clone();
                    let card_id = card.id.clone();
                    app.start_insert(Box::new(MarkdownEditor::new(card_id, &desc)));
                }
        }
        Action::EditTitle => {
            if let Some(board) = app.board()
                && let Some(card) = board.current_card() {
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
                && let Some(list) = board.lists.get(board.selected_list) {
                    let name = list.name.clone();
                    let list_id = list.id.clone();
                    app.start_insert(Box::new(line_editor::RenameList::new(list_id, &name)));
                }
        }
        Action::ArchiveList => {
            if let Some(board) = app.board()
                && !board.lists.is_empty() {
                    app.open_dialog(Box::new(ConfirmArchiveList));
                }
        }
        Action::MoveListLeft => move_list_in_direction(app, MoveDir::Left)?,
        Action::MoveListRight => move_list_in_direction(app, MoveDir::Right)?,
        Action::ArchiveCard => {
            if let Some(board) = app.board()
                && board.current_card_id().is_some() {
                    app.open_dialog(Box::new(ConfirmArchiveCard));
                }
        }
        Action::ViewArchivedCards => {
            if let Some(editor) = &app.editor {
                let archived = editor.archived_cards();
                if archived.is_empty() {
                    app.set_status("No archived cards".into());
                } else {
                    app.open_dialog(Box::new(ArchivedCards {
                        cards: archived,
                        selected: 0,
                    }));
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
                && board.current_card_id().is_some() {
                    app.open_dialog(Box::new(LabelPicker { selected_idx: 0 }));
                }
        }
        Action::ManageLabels => {
            app.open_dialog(Box::new(LabelManager { selected_idx: 0 }));
        }
        Action::SetDueDate => {
            if let Some(board) = app.board()
                && let Some(card) = board.current_card() {
                    let date_str = card
                        .due_date
                        .map(|d| d.format("%Y-%m-%d").to_string())
                        .unwrap_or_default();
                    let card_id = card.id.clone();
                    app.start_insert(Box::new(DatePicker::new(card_id, &date_str, InsertSurface::BoardView)));
                }
        }
        Action::CardHistoryDialog => {
            if let Some(board) = app.board()
                && board.current_card_id().is_some() {
                    app.open_dialog(Box::new(CardHistory { scroll: 0 }));
                }
        }
        Action::ClearDueDate => {
            if let Some(board) = app.board()
                && let Some(card_id) = board.current_card_id().cloned() {
                    app.apply(Command::ClearDueDate { card_id })?;
                    app.set_status("Due date cleared".into());
                }
        }
    }
    Ok(())
}

fn move_list_in_direction(app: &mut App, direction: MoveDir) -> anyhow::Result<()> {
    let list_id = app
        .board()
        .and_then(|b| b.lists.get(b.selected_list).map(|l| l.id.clone()));
    if let Some(list_id) = list_id {
        app.apply(Command::MoveList { list_id, direction })?;
    }
    Ok(())
}

fn move_card_in_direction(app: &mut App, direction: MoveDir) -> anyhow::Result<()> {
    let card_id = match app
        .board()
        .and_then(|b| b.current_card_id().cloned())
    {
        Some(id) => id,
        None => return Ok(()),
    };
    app.apply(Command::MoveCard { card_id, direction })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::model::board::BoardMeta;
    use crate::model::card::Card;
    use crate::model::list::CardList;
    use crate::storage::{board_store, card_store, list_store};
    use crate::test_support::with_temp_dir;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn shift_key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    /// Three boards in display order, with the first one open.
    fn three_boards_first_open() -> (App, Vec<BoardMeta>) {
        let metas: Vec<BoardMeta> = ["A", "B", "C"]
            .iter()
            .map(|n| {
                let m = board_store::create_board((*n).into()).unwrap();
                board_store::append_to_order(&m.id).unwrap();
                m
            })
            .collect();
        let app = App::new(Some(metas[0].id.clone())).unwrap();
        (app, metas)
    }

    fn open_board_id(app: &App) -> crate::model::ids::ShortId {
        app.board().unwrap().meta.id.clone()
    }

    /// Build an App with a single list of cards with the given titles.
    fn single_list_fixture(titles: &[&str]) -> (App, BoardMeta) {
        let mut meta = board_store::create_board("Board".into()).unwrap();
        let mut list = CardList::new("L".into());
        for t in titles {
            let c = Card::new((*t).into());
            card_store::save_card(&meta.id, &c).unwrap();
            list.card_ids.push(c.id.clone());
        }
        list_store::save_list(&meta.id, &list).unwrap();
        meta.list_order = vec![list.id.clone()];
        board_store::save_board(&meta).unwrap();
        (App::new(Some(meta.id.clone())).unwrap(), meta)
    }

    /// Build an App with a board having two lists; list 0 has 3 cards, list 1 has 1.
    fn fixture() -> (App, BoardMeta, Vec<Card>, Vec<Card>) {
        let mut meta = board_store::create_board("Board".into()).unwrap();
        let mut list_a = CardList::new("A".into());
        let mut list_b = CardList::new("B".into());

        let a_cards: Vec<Card> = (0..3).map(|i| Card::new(format!("a{i}"))).collect();
        let b_cards = vec![Card::new("b0".into())];

        for c in &a_cards {
            card_store::save_card(&meta.id, c).unwrap();
            list_a.card_ids.push(c.id.clone());
        }
        for c in &b_cards {
            card_store::save_card(&meta.id, c).unwrap();
            list_b.card_ids.push(c.id.clone());
        }
        list_store::save_list(&meta.id, &list_a).unwrap();
        list_store::save_list(&meta.id, &list_b).unwrap();
        meta.list_order = vec![list_a.id.clone(), list_b.id.clone()];
        board_store::save_board(&meta).unwrap();

        let app = App::new(Some(meta.id.clone())).unwrap();
        (app, meta, a_cards, b_cards)
    }

    #[test]
    fn down_moves_selection() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            assert_eq!(app.board().unwrap().selected_card[0], 0);
            handle(&mut app, key(KeyCode::Down)).unwrap();
            assert_eq!(app.board().unwrap().selected_card[0], 1);
        });
    }

    #[test]
    fn down_stops_at_last_card() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            for _ in 0..10 {
                handle(&mut app, key(KeyCode::Down)).unwrap();
            }
            // Max for list 0 (3 cards) is index 2
            assert_eq!(app.board().unwrap().selected_card[0], 2);
        });
    }

    #[test]
    fn up_stops_at_zero() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            for _ in 0..5 {
                handle(&mut app, key(KeyCode::Up)).unwrap();
            }
            assert_eq!(app.board().unwrap().selected_card[0], 0);
        });
    }

    #[test]
    fn right_moves_to_next_list() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            handle(&mut app, key(KeyCode::Right)).unwrap();
            assert_eq!(app.board().unwrap().selected_list, 1);
        });
    }

    #[test]
    fn right_stops_at_last_list() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            for _ in 0..5 {
                handle(&mut app, key(KeyCode::Right)).unwrap();
            }
            assert_eq!(app.board().unwrap().selected_list, 1);
        });
    }

    #[test]
    fn left_stops_at_zero() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            for _ in 0..5 {
                handle(&mut app, key(KeyCode::Left)).unwrap();
            }
            assert_eq!(app.board().unwrap().selected_list, 0);
        });
    }

    #[test]
    fn g_jumps_to_top() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            app.board_mut().unwrap().selected_card[0] = 2;
            handle(&mut app, key(KeyCode::Char('g'))).unwrap();
            assert_eq!(app.board().unwrap().selected_card[0], 0);
        });
    }

    #[test]
    fn shift_g_jumps_to_bottom() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            handle(&mut app, shift_key(KeyCode::Char('G'))).unwrap();
            assert_eq!(app.board().unwrap().selected_card[0], 2);
        });
    }

    #[test]
    fn shift_down_moves_card_within_list() {
        with_temp_dir(|| {
            let (mut app, _, a_cards, _) = fixture();
            // Cards initially [a0, a1, a2]; selection at 0
            handle(&mut app, shift_key(KeyCode::Down)).unwrap();
            let board = app.board().unwrap();
            assert_eq!(board.lists[0].card_ids[0], a_cards[1].id);
            assert_eq!(board.lists[0].card_ids[1], a_cards[0].id);
            assert_eq!(board.selected_card[0], 1);
        });
    }

    #[test]
    fn shift_up_at_top_is_noop() {
        with_temp_dir(|| {
            let (mut app, _, a_cards, _) = fixture();
            handle(&mut app, shift_key(KeyCode::Up)).unwrap();
            let board = app.board().unwrap();
            assert_eq!(board.lists[0].card_ids[0], a_cards[0].id);
            assert_eq!(board.selected_card[0], 0);
        });
    }

    #[test]
    fn shift_down_at_bottom_is_noop() {
        with_temp_dir(|| {
            let (mut app, _, a_cards, _) = fixture();
            app.board_mut().unwrap().selected_card[0] = 2;
            handle(&mut app, shift_key(KeyCode::Down)).unwrap();
            let board = app.board().unwrap();
            assert_eq!(board.lists[0].card_ids[2], a_cards[2].id);
            assert_eq!(board.selected_card[0], 2);
        });
    }

    #[test]
    fn shift_right_moves_card_to_next_list() {
        with_temp_dir(|| {
            let (mut app, _, a_cards, _) = fixture();
            handle(&mut app, shift_key(KeyCode::Right)).unwrap();
            let board = app.board().unwrap();
            // a0 moved to list 1 at index 0
            assert_eq!(board.lists[0].card_ids.len(), 2);
            assert!(board.lists[1].card_ids.contains(&a_cards[0].id));
            assert_eq!(board.selected_list, 1);
        });
    }

    #[test]
    fn shift_left_from_first_list_is_noop() {
        with_temp_dir(|| {
            let (mut app, _, a_cards, _) = fixture();
            handle(&mut app, shift_key(KeyCode::Left)).unwrap();
            let board = app.board().unwrap();
            assert_eq!(board.lists[0].card_ids.len(), 3);
            assert_eq!(board.lists[0].card_ids[0], a_cards[0].id);
        });
    }

    #[test]
    fn shift_right_from_last_list_is_noop() {
        with_temp_dir(|| {
            let (mut app, _, _, b_cards) = fixture();
            handle(&mut app, key(KeyCode::Right)).unwrap();
            assert_eq!(app.board().unwrap().selected_list, 1);
            handle(&mut app, shift_key(KeyCode::Right)).unwrap();
            let board = app.board().unwrap();
            assert_eq!(board.lists[1].card_ids.len(), 1);
            assert_eq!(board.lists[1].card_ids[0], b_cards[0].id);
        });
    }

    #[test]
    fn q_sets_should_quit() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            handle(&mut app, key(KeyCode::Char('q'))).unwrap();
            assert!(app.should_quit);
        });
    }

    #[test]
    fn question_enters_help() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            handle(&mut app, key(KeyCode::Char('?'))).unwrap();
            assert_eq!(app.mode, AppMode::Help);
        });
    }

    #[test]
    fn b_returns_to_board_selector() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            handle(&mut app, key(KeyCode::Char('b'))).unwrap();
            assert!(app.editor.is_none());
            assert_eq!(app.mode, AppMode::BoardSelector);
        });
    }

    #[test]
    fn enter_opens_card_detail() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            handle(&mut app, key(KeyCode::Enter)).unwrap();
            assert_eq!(app.mode, AppMode::CardDetail);
        });
    }

    #[test]
    fn slash_enters_command_and_clears_query() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            app.search_query = "old".into();
            handle(&mut app, key(KeyCode::Char('/'))).unwrap();
            assert_eq!(app.mode, AppMode::Command);
            assert!(app.search_query.is_empty());
        });
    }

    #[test]
    fn shift_f_clears_filters() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            app.search_active = true;
            app.search_query = "x".into();
            app.label_filter = Some(crate::model::label::LabelColor::Red);
            handle(&mut app, shift_key(KeyCode::Char('F'))).unwrap();
            assert!(!app.search_active);
            assert!(app.search_query.is_empty());
            assert!(app.label_filter.is_none());
        });
    }

    #[test]
    fn shift_u_clears_due_date() {
        with_temp_dir(|| {
            let (mut app, meta, a_cards, _) = fixture();
            // Set a due date on selected card
            let cid = a_cards[0].id.clone();
            {
                let board = app.board_mut().unwrap();
                let card = board.cards.get_mut(&cid).unwrap();
                card.due_date = Some(chrono::NaiveDate::from_ymd_opt(2099, 1, 1).unwrap());
                card_store::save_card(&meta.id, card).unwrap();
            }
            handle(&mut app, shift_key(KeyCode::Char('U'))).unwrap();
            let board = app.board().unwrap();
            assert!(board.cards.get(&cid).unwrap().due_date.is_none());
            // Verify persisted
            let on_disk = card_store::load_card(&meta.id, &cid).unwrap();
            assert!(on_disk.due_date.is_none());
        });
    }

    #[test]
    fn search_nav_skips_non_matching_cards() {
        with_temp_dir(|| {
            let (mut app, _) = single_list_fixture(&["alpha", "BINGO match", "gamma"]);
            app.search_active = true;
            app.search_query = "BINGO".into();
            // From index 0, next match is index 1
            handle(&mut app, key(KeyCode::Down)).unwrap();
            assert_eq!(app.board().unwrap().selected_card[0], 1);
            // From index 1, no further match → stays at 1
            handle(&mut app, key(KeyCode::Down)).unwrap();
            assert_eq!(app.board().unwrap().selected_card[0], 1);
        });
    }

    #[test]
    fn search_nav_up_finds_previous_match() {
        with_temp_dir(|| {
            let (mut app, _) = single_list_fixture(&["BINGO one", "nope", "BINGO three"]);
            app.search_active = true;
            app.search_query = "BINGO".into();
            app.board_mut().unwrap().selected_card[0] = 2;
            handle(&mut app, key(KeyCode::Up)).unwrap();
            assert_eq!(app.board().unwrap().selected_card[0], 0);
        });
    }

    #[test]
    fn shift_left_arrow_swaps_lists() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            handle(&mut app, key(KeyCode::Right)).unwrap(); // move to list 1
            handle(&mut app, shift_key(KeyCode::Char('<'))).unwrap();
            let board = app.board().unwrap();
            assert_eq!(board.lists[0].name, "B");
            assert_eq!(board.lists[1].name, "A");
            assert_eq!(board.selected_list, 0);
        });
    }

    #[test]
    fn shift_right_arrow_swaps_lists() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            handle(&mut app, shift_key(KeyCode::Char('>'))).unwrap();
            let board = app.board().unwrap();
            assert_eq!(board.lists[0].name, "B");
            assert_eq!(board.lists[1].name, "A");
            assert_eq!(board.selected_list, 1);
        });
    }

    #[test]
    fn t_starts_inline_title_edit() {
        with_temp_dir(|| {
            let (mut app, _, a_cards, _) = fixture();
            handle(&mut app, key(KeyCode::Char('t'))).unwrap();
            assert!(matches!(app.mode, AppMode::Insert));
            // Title pre-filled in the handler's buffer
            let h = app.insert.as_ref().unwrap();
            assert_eq!(h.line_buffer(), Some(a_cards[0].title.as_str()));
            // Return path: previous_mode set to Normal
            assert_eq!(app.previous_mode, Some(AppMode::Normal));
        });
    }

    #[test]
    fn e_starts_description_edit() {
        with_temp_dir(|| {
            let (mut app, meta, a_cards, _) = fixture();
            // Pre-populate description on selected card
            let cid = a_cards[0].id.clone();
            {
                let board = app.board_mut().unwrap();
                let card = board.cards.get_mut(&cid).unwrap();
                card.description = "hello desc".into();
                card_store::save_card(&meta.id, card).unwrap();
            }
            handle(&mut app, key(KeyCode::Char('e'))).unwrap();
            assert!(matches!(app.mode, AppMode::Insert));
            let editor = app
                .insert
                .as_ref()
                .unwrap()
                .as_any()
                .downcast_ref::<crate::insert::markdown_editor::MarkdownEditor>()
                .expect("description editor active");
            assert_eq!(editor.input.current_text(), "hello desc");
            assert_eq!(app.previous_mode, Some(AppMode::Normal));
        });
    }

    #[test]
    fn k_switches_to_next_board() {
        with_temp_dir(|| {
            let (mut app, metas) = three_boards_first_open();
            assert_eq!(open_board_id(&app), metas[0].id);
            handle(&mut app, key(KeyCode::Char('k'))).unwrap();
            assert_eq!(open_board_id(&app), metas[1].id);
            assert_eq!(app.selected_board_idx, 1);
            assert_eq!(app.mode, AppMode::Normal);
        });
    }

    #[test]
    fn j_wraps_to_last_board() {
        with_temp_dir(|| {
            let (mut app, metas) = three_boards_first_open();
            handle(&mut app, key(KeyCode::Char('j'))).unwrap();
            assert_eq!(open_board_id(&app), metas[2].id);
            assert_eq!(app.selected_board_idx, 2);
        });
    }

    #[test]
    fn k_wraps_from_last_to_first() {
        with_temp_dir(|| {
            let (mut app, metas) = three_boards_first_open();
            app.load_board(&metas[2].id).unwrap();
            app.selected_board_idx = 2;
            handle(&mut app, key(KeyCode::Char('k'))).unwrap();
            assert_eq!(open_board_id(&app), metas[0].id);
            assert_eq!(app.selected_board_idx, 0);
        });
    }

    #[test]
    fn j_k_are_noop_with_single_board() {
        with_temp_dir(|| {
            let m = board_store::create_board("Solo".into()).unwrap();
            board_store::append_to_order(&m.id).unwrap();
            let mut app = App::new(Some(m.id.clone())).unwrap();
            handle(&mut app, key(KeyCode::Char('j'))).unwrap();
            assert_eq!(open_board_id(&app), m.id);
            handle(&mut app, key(KeyCode::Char('k'))).unwrap();
            assert_eq!(open_board_id(&app), m.id);
        });
    }
}
