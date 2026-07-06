use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, AppMode};
use crate::command::Command;
use crate::dialog::{
    card_history::CardHistory, label_manager::LabelManager, label_picker::LabelPicker,
};
use crate::insert::{
    date_picker::DatePicker, line_editor, markdown_editor::MarkdownEditor, InsertSurface,
};

use super::keymap::{self, Binding};

#[derive(Clone, Copy)]
pub enum Action {
    Close,
    ItemUp,
    ItemDown,
    ScrollDown,
    ScrollUp,
    ReorderItemDown,
    ReorderItemUp,
    ToggleItem,
    AddItem,
    DeleteItem,
    EditItem,
    EditDescription,
    EditTitle,
    CopyDescription,
    CopyChecklist,
    AssignLabels,
    ManageLabels,
    SetDueDate,
    ClearDueDate,
    HistoryDialog,
    Help,
    Quit,
}

/// Card-detail keymap. Single definition of key → action → help text; the
/// help overlay renders from this table.
pub static KEYMAP: &[Binding<Action>] = &[
    // Card
    Binding { code: KeyCode::Char('t'), shift: None, action: Action::EditTitle, keys: "t", help: "Edit title", section: "Card" },
    Binding { code: KeyCode::Char('e'), shift: None, action: Action::EditDescription, keys: "e", help: "Edit description", section: "Card" },
    Binding { code: KeyCode::PageUp, shift: None, action: Action::ScrollUp, keys: "PgUp/PgDn ←/→", help: "Scroll description", section: "Card" },
    Binding { code: KeyCode::PageDown, shift: None, action: Action::ScrollDown, keys: "PgUp/PgDn ←/→", help: "Scroll description", section: "Card" },
    Binding { code: KeyCode::Left, shift: None, action: Action::ScrollUp, keys: "PgUp/PgDn ←/→", help: "Scroll description", section: "Card" },
    Binding { code: KeyCode::Right, shift: None, action: Action::ScrollDown, keys: "PgUp/PgDn ←/→", help: "Scroll description", section: "Card" },
    Binding { code: KeyCode::Char('y'), shift: None, action: Action::CopyDescription, keys: "y", help: "Copy description", section: "Card" },
    Binding { code: KeyCode::Char('Y'), shift: None, action: Action::CopyChecklist, keys: "Y", help: "Copy checklist (md)", section: "Card" },
    Binding { code: KeyCode::Char('h'), shift: None, action: Action::HistoryDialog, keys: "h", help: "View change history", section: "Card" },
    // Checklist
    Binding { code: KeyCode::Up, shift: Some(false), action: Action::ItemUp, keys: "Up / Down", help: "Navigate items", section: "Checklist" },
    Binding { code: KeyCode::Down, shift: Some(false), action: Action::ItemDown, keys: "Up / Down", help: "Navigate items", section: "Checklist" },
    Binding { code: KeyCode::Up, shift: Some(true), action: Action::ReorderItemUp, keys: "Shift+Up/Down", help: "Reorder item", section: "Checklist" },
    Binding { code: KeyCode::Down, shift: Some(true), action: Action::ReorderItemDown, keys: "Shift+Up/Down", help: "Reorder item", section: "Checklist" },
    Binding { code: KeyCode::Char(' '), shift: None, action: Action::ToggleItem, keys: "Space", help: "Toggle item", section: "Checklist" },
    Binding { code: KeyCode::Char('a'), shift: None, action: Action::AddItem, keys: "a", help: "Add item", section: "Checklist" },
    Binding { code: KeyCode::Enter, shift: None, action: Action::EditItem, keys: "Enter", help: "Edit item", section: "Checklist" },
    Binding { code: KeyCode::Char('x'), shift: None, action: Action::DeleteItem, keys: "x", help: "Delete item", section: "Checklist" },
    // Labels & Due
    Binding { code: KeyCode::Char('l'), shift: None, action: Action::AssignLabels, keys: "l", help: "Assign / remove labels", section: "Labels & Due" },
    Binding { code: KeyCode::Char('L'), shift: None, action: Action::ManageLabels, keys: "L", help: "Manage labels", section: "Labels & Due" },
    Binding { code: KeyCode::Char('u'), shift: None, action: Action::SetDueDate, keys: "u", help: "Set due date", section: "Labels & Due" },
    Binding { code: KeyCode::Char('U'), shift: None, action: Action::ClearDueDate, keys: "U", help: "Clear due date", section: "Labels & Due" },
    // App
    Binding { code: KeyCode::Esc, shift: None, action: Action::Close, keys: "Esc", help: "Close", section: "App" },
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
        Action::Close => {
            if let Some(editor) = &mut app.editor {
                editor.reset_detail_cursor();
            }
            app.mode = AppMode::Normal;
        }
        Action::ItemDown => {
            if let Some(editor) = &mut app.editor {
                editor.detail_item_down();
            }
        }
        Action::ItemUp => {
            if let Some(editor) = &mut app.editor {
                editor.detail_item_up();
            }
        }
        Action::ScrollDown => scroll_description(app, 1, true),
        Action::ScrollUp => scroll_description(app, 1, false),
        Action::ReorderItemDown => {
            if let Some(board) = app.board()
                && let Some(card_id) = board.current_card_id().cloned()
                && let Some(card) = board.cards.get(&card_id) {
                    let ii = board.detail_item_idx;
                    if ii + 1 < card.checklist.len() {
                        app.apply(Command::ReorderChecklistItem {
                            card_id,
                            from: ii,
                            to: ii + 1,
                        })?;
                    }
                }
        }
        Action::ReorderItemUp => {
            if let Some(board) = app.board()
                && let Some(card_id) = board.current_card_id().cloned() {
                    let ii = board.detail_item_idx;
                    if ii > 0 {
                        app.apply(Command::ReorderChecklistItem {
                            card_id,
                            from: ii,
                            to: ii - 1,
                        })?;
                    }
                }
        }
        Action::ToggleItem => {
            if let Some(board) = app.board()
                && let Some(card_id) = board.current_card_id().cloned()
                && let Some(card) = board.cards.get(&card_id) {
                    let ii = board.detail_item_idx;
                    if ii < card.checklist.len() {
                        app.apply(Command::ToggleChecklistItem { card_id, item_idx: ii })?;
                    }
                }
        }
        Action::AddItem => {
            if let Some(card_id) =
                app.board().and_then(|b| b.current_card_id().cloned())
            {
                app.start_insert(Box::new(line_editor::NewChecklistItem::new(card_id)));
            }
        }
        Action::DeleteItem => {
            if let Some(board) = app.board()
                && let Some(card_id) = board.current_card_id().cloned()
                && let Some(card) = board.cards.get(&card_id) {
                    let ii = board.detail_item_idx;
                    if ii < card.checklist.len() {
                        app.apply(Command::RemoveChecklistItem { card_id, item_idx: ii })?;
                    }
                }
        }
        Action::EditItem => {
            if let Some(board) = app.board()
                && let Some(card_id) = board.current_card_id().cloned()
                && let Some(card) = board.cards.get(&card_id) {
                    let ii = board.detail_item_idx;
                    if let Some(item) = card.checklist.get(ii) {
                        let text = item.text.clone();
                        app.start_insert(Box::new(line_editor::EditChecklistItem::new(
                            card_id, ii, &text,
                        )));
                    }
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
                        card_id, &title, false,
                    )));
                }
        }
        Action::CopyDescription => {
            if let Some(board) = app.board()
                && let Some(card) = board.current_card() {
                    let desc = card.description.clone();
                    let what = format!("description of '{}'", card.title);
                    app.copy_to_clipboard(desc, &what);
                }
        }
        Action::CopyChecklist => {
            if let Some(board) = app.board()
                && let Some(card) = board.current_card() {
                    if card.checklist.is_empty() {
                        app.set_status("Checklist is empty".into());
                    } else {
                        let md = card.checklist_as_markdown();
                        let what = format!("checklist of '{}'", card.title);
                        app.copy_to_clipboard(md, &what);
                    }
                }
        }
        Action::AssignLabels => {
            if let Some(board) = app.board()
                && board.current_card_id().is_some() {
                    app.open_dialog(Box::new(LabelPicker { selected_idx: 0 }));
                }
        }
        Action::ManageLabels => {
            app.open_dialog(Box::new(LabelManager { selected_idx: 0, from_picker: false }));
        }
        Action::SetDueDate => {
            if let Some(board) = app.board()
                && let Some(card) = board.current_card() {
                    let date_str = card
                        .due_date
                        .map(|d| d.format("%Y-%m-%d").to_string())
                        .unwrap_or_default();
                    let card_id = card.id.clone();
                    app.start_insert(Box::new(DatePicker::new(card_id, &date_str, InsertSurface::CardDetail)));
                }
        }
        Action::ClearDueDate => {
            if let Some(board) = app.board()
                && let Some((card_id, title)) = board
                    .current_card()
                    .map(|c| (c.id.clone(), c.title.clone()))
            {
                app.apply(Command::ClearDueDate { card_id })?;
                app.set_status(format!("Cleared due date of '{title}'"));
            }
        }
        Action::HistoryDialog => {
            if let Some(board) = app.board()
                && board.current_card_id().is_some() {
                    app.open_dialog(Box::new(CardHistory { scroll: 0 }));
                }
        }
        Action::Help => {
            app.remember_return_mode();
            app.mode = AppMode::Help;
        }
        Action::Quit => {
            app.should_quit = true;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, AppMode};
    use crate::model::card::{Card, ChecklistItem};
    use crate::model::list::CardList;
    use crate::storage::{board_store, card_store, list_store};
    use crate::test_support::with_temp_dir;

    fn setup_card_detail() -> App {
        let mut meta = board_store::create_board("Test".into()).unwrap();
        let mut list = CardList::new("To Do".into());
        let mut card = Card::new("Card".into());
        card.checklist = vec![
            ChecklistItem { text: "a".into(), completed: false },
            ChecklistItem { text: "b".into(), completed: false },
            ChecklistItem { text: "c".into(), completed: false },
        ];
        card.due_date = Some(chrono::NaiveDate::from_ymd_opt(2099, 1, 1).unwrap());
        card_store::save_card(&meta.id, &card).unwrap();
        list.card_ids.push(card.id.clone());
        list_store::save_list(&meta.id, &list).unwrap();
        meta.list_order = vec![list.id.clone()];
        board_store::save_board(&meta).unwrap();

        let mut app = App::new(Some(meta.id)).unwrap();
        app.mode = AppMode::CardDetail;
        app
    }

    fn press(app: &mut App, code: KeyCode) {
        handle(app, KeyEvent::new(code, KeyModifiers::empty())).unwrap();
    }

    fn press_shift(app: &mut App, code: KeyCode) {
        handle(app, KeyEvent::new(code, KeyModifiers::SHIFT)).unwrap();
    }

    #[test]
    fn esc_returns_to_normal_mode_and_resets_scroll() {
        with_temp_dir(|| {
            let mut app = setup_card_detail();
            app.board_mut().unwrap().detail_scroll = 5;
            app.board_mut().unwrap().detail_item_idx = 2;

            press(&mut app, KeyCode::Esc);

            assert!(matches!(app.mode, AppMode::Normal));
            let board = app.board().unwrap();
            assert_eq!(board.detail_scroll, 0);
            assert_eq!(board.detail_item_idx, 0);
        });
    }

    #[test]
    fn down_arrow_navigates_checklist_with_clamp() {
        with_temp_dir(|| {
            let mut app = setup_card_detail();
            press(&mut app, KeyCode::Down);
            assert_eq!(app.board().unwrap().detail_item_idx, 1);
            press(&mut app, KeyCode::Down);
            assert_eq!(app.board().unwrap().detail_item_idx, 2);
            // Already at last item — clamp
            press(&mut app, KeyCode::Down);
            assert_eq!(app.board().unwrap().detail_item_idx, 2);
        });
    }

    #[test]
    fn up_arrow_stops_at_zero() {
        with_temp_dir(|| {
            let mut app = setup_card_detail();
            app.board_mut().unwrap().detail_item_idx = 1;
            press(&mut app, KeyCode::Up);
            assert_eq!(app.board().unwrap().detail_item_idx, 0);
            press(&mut app, KeyCode::Up);
            assert_eq!(app.board().unwrap().detail_item_idx, 0);
        });
    }

    #[test]
    fn space_toggles_current_checklist_item() {
        with_temp_dir(|| {
            let mut app = setup_card_detail();
            press(&mut app, KeyCode::Char(' '));
            let card = app.board().unwrap().current_card().unwrap();
            assert!(card.checklist[0].completed);
            press(&mut app, KeyCode::Char(' '));
            let card = app.board().unwrap().current_card().unwrap();
            assert!(!card.checklist[0].completed);
        });
    }

    #[test]
    fn x_deletes_current_checklist_item_and_clamps_selection() {
        with_temp_dir(|| {
            let mut app = setup_card_detail();
            app.board_mut().unwrap().detail_item_idx = 2; // last
            press(&mut app, KeyCode::Char('x'));
            let board = app.board().unwrap();
            assert_eq!(board.current_card().unwrap().checklist.len(), 2);
            // selection moved back to new last
            assert_eq!(board.detail_item_idx, 1);
        });
    }

    #[test]
    fn shift_down_reorders_checklist_item() {
        with_temp_dir(|| {
            let mut app = setup_card_detail();
            press_shift(&mut app, KeyCode::Down);
            let board = app.board().unwrap();
            assert_eq!(board.detail_item_idx, 1);
            let cl = &board.current_card().unwrap().checklist;
            assert_eq!(cl[0].text, "b");
            assert_eq!(cl[1].text, "a");
            assert_eq!(cl[2].text, "c");
        });
    }

    #[test]
    fn capital_u_clears_due_date_and_logs() {
        with_temp_dir(|| {
            let mut app = setup_card_detail();
            press_shift(&mut app, KeyCode::Char('U'));
            let card = app.board().unwrap().current_card().unwrap();
            assert!(card.due_date.is_none());
            // Card history should include "Cleared due date"
            assert!(card.history.iter().any(|h| h.action.contains("Cleared")));
        });
    }

    #[test]
    fn l_lowercase_opens_label_picker_dialog() {
        with_temp_dir(|| {
            let mut app = setup_card_detail();
            press(&mut app, KeyCode::Char('l'));
            assert!(matches!(app.mode, AppMode::Dialog));
            assert!(app.dialog.is_some());
        });
    }

    #[test]
    fn l_uppercase_opens_label_manager_dialog() {
        with_temp_dir(|| {
            let mut app = setup_card_detail();
            press_shift(&mut app, KeyCode::Char('L'));
            assert!(matches!(app.mode, AppMode::Dialog));
            assert!(app.dialog.is_some());
        });
    }

    #[test]
    fn t_starts_card_title_edit_with_prefill() {
        with_temp_dir(|| {
            let mut app = setup_card_detail();
            press(&mut app, KeyCode::Char('t'));
            assert!(matches!(app.mode, AppMode::Insert));
            let h = app.insert.as_ref().unwrap();
            assert_eq!(h.line_buffer(), Some("Card"));
        });
    }

    #[test]
    fn e_starts_description_editor() {
        with_temp_dir(|| {
            let mut app = setup_card_detail();
            press(&mut app, KeyCode::Char('e'));
            assert!(matches!(app.mode, AppMode::Insert));
            assert!(
                app.insert
                    .as_ref()
                    .unwrap()
                    .as_any()
                    .downcast_ref::<crate::insert::markdown_editor::MarkdownEditor>()
                    .is_some()
            );
        });
    }

    #[test]
    fn h_opens_card_history_dialog() {
        with_temp_dir(|| {
            let mut app = setup_card_detail();
            press(&mut app, KeyCode::Char('h'));
            assert!(matches!(app.mode, AppMode::Dialog));
            assert!(app.dialog.is_some());
        });
    }

    #[test]
    fn unmatched_keys_are_noop() {
        with_temp_dir(|| {
            let mut app = setup_card_detail();
            let before_mode = app.mode.clone();
            // Use a key not bound in this mode.
            press(&mut app, KeyCode::Char('z'));
            assert!(matches!(app.mode, AppMode::CardDetail));
            assert_eq!(
                std::mem::discriminant(&app.mode),
                std::mem::discriminant(&before_mode)
            );
        });
    }

    #[test]
    fn enter_on_existing_item_starts_edit_with_prefill() {
        with_temp_dir(|| {
            let mut app = setup_card_detail();
            app.board_mut().unwrap().detail_item_idx = 1; // "b"
            press(&mut app, KeyCode::Enter);
            assert!(matches!(app.mode, AppMode::Insert));
            let h = app.insert.as_ref().unwrap();
            assert_eq!(h.line_buffer(), Some("b"));
        });
    }

    #[test]
    fn page_down_scrolls_description() {
        with_temp_dir(|| {
            let mut app = setup_card_detail();
            // The renderer reports the effective max scroll; simulate it.
            app.board().unwrap().detail_max_scroll.set(20);
            press(&mut app, KeyCode::PageDown);
            assert!(app.board().unwrap().detail_scroll > 0);
            press(&mut app, KeyCode::PageUp);
            assert_eq!(app.board().unwrap().detail_scroll, 0);
        });
    }

    #[test]
    fn arrow_keys_scroll_description() {
        with_temp_dir(|| {
            let mut app = setup_card_detail();
            app.board().unwrap().detail_max_scroll.set(20);
            press(&mut app, KeyCode::Right);
            assert_eq!(app.board().unwrap().detail_scroll, 1);
            press(&mut app, KeyCode::Left);
            assert_eq!(app.board().unwrap().detail_scroll, 0);
        });
    }

    #[test]
    fn scroll_up_responds_immediately_after_overscroll() {
        with_temp_dir(|| {
            let mut app = setup_card_detail();
            // Stored scroll ran past the rendered max (e.g. layout change).
            app.board().unwrap().detail_max_scroll.set(10);
            app.board_mut().unwrap().detail_scroll = 50;
            press(&mut app, KeyCode::PageUp);
            assert_eq!(app.board().unwrap().detail_scroll, 9);
        });
    }

    #[test]
    fn scroll_down_clamps_to_rendered_max() {
        with_temp_dir(|| {
            let mut app = setup_card_detail();
            app.board().unwrap().detail_max_scroll.set(3);
            for _ in 0..10 {
                press(&mut app, KeyCode::PageDown);
            }
            assert_eq!(app.board().unwrap().detail_scroll, 3);
        });
    }
}

fn scroll_description(app: &mut App, step: usize, down: bool) {
    let max_scroll = app
        .board()
        .map(|b| b.detail_max_scroll.get())
        .unwrap_or(0);
    if let Some(editor) = &mut app.editor {
        editor.scroll_detail(step, down, max_scroll);
    }
}
