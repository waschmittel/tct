use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, AppMode};
use crate::command::Command;
use crate::dialog::{
    card_history::CardHistory, label_manager::LabelManager, label_picker::LabelPicker,
};
use crate::insert::{
    date_picker::DatePicker, line_editor, markdown_editor::MarkdownEditor,
};

pub fn handle(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);

    match (key.code, shift) {
        (KeyCode::Esc, _) => {
            if let Some(board) = &mut app.board {
                board.detail_item_idx = 0;
                board.detail_scroll = 0;
            }
            app.mode = AppMode::Normal;
        }

        // Checklist item navigation
        (KeyCode::Down, false) => {
            if let Some(board) = &mut app.board
                && let Some(card) = board.current_card()
                    && board.detail_item_idx < card.checklist.len().saturating_sub(1) {
                        board.detail_item_idx += 1;
                    }
        }
        (KeyCode::Up, false) => {
            if let Some(board) = &mut app.board
                && board.detail_item_idx > 0 {
                    board.detail_item_idx -= 1;
                }
        }

        // Description scrolling
        (KeyCode::PageDown, _) => {
            scroll_description(app, 5, true);
        }
        (KeyCode::PageUp, _) => {
            scroll_description(app, 5, false);
        }

        // Reorder checklist item down
        (KeyCode::Down, true) => {
            if let Some(board) = &app.board
                && let Some(card_id) = board.current_card_id().cloned()
                && let Some(card) = board.cards.get(&card_id) {
                    let ii = board.detail_item_idx;
                    if ii + 1 < card.checklist.len() {
                        app.apply(Command::ReorderChecklistItem {
                            card_id,
                            from: ii,
                            to: ii + 1,
                        })?;
                        if let Some(b) = &mut app.board {
                            b.detail_item_idx = ii + 1;
                        }
                    }
                }
        }

        // Reorder checklist item up
        (KeyCode::Up, true) => {
            if let Some(board) = &app.board
                && let Some(card_id) = board.current_card_id().cloned() {
                    let ii = board.detail_item_idx;
                    if ii > 0 {
                        app.apply(Command::ReorderChecklistItem {
                            card_id,
                            from: ii,
                            to: ii - 1,
                        })?;
                        if let Some(b) = &mut app.board {
                            b.detail_item_idx = ii - 1;
                        }
                    }
                }
        }

        // Toggle checklist item
        (KeyCode::Char(' '), _) => {
            if let Some(board) = &app.board
                && let Some(card_id) = board.current_card_id().cloned()
                && let Some(card) = board.cards.get(&card_id) {
                    let ii = board.detail_item_idx;
                    if ii < card.checklist.len() {
                        app.apply(Command::ToggleChecklistItem { card_id, item_idx: ii })?;
                    }
                }
        }

        // Add checklist item
        (KeyCode::Char('a'), _) => {
            if let Some(card_id) =
                app.board.as_ref().and_then(|b| b.current_card_id().cloned())
            {
                app.start_insert(Box::new(line_editor::NewChecklistItem::new(card_id)));
            }
        }

        // Delete checklist item
        (KeyCode::Char('x'), _) => {
            if let Some(board) = &app.board
                && let Some(card_id) = board.current_card_id().cloned()
                && let Some(card) = board.cards.get(&card_id) {
                    let ii = board.detail_item_idx;
                    if ii < card.checklist.len() {
                        app.apply(Command::RemoveChecklistItem { card_id: card_id.clone(), item_idx: ii })?;
                        if let Some(b) = &mut app.board
                            && let Some(c) = b.cards.get(&card_id) {
                                if b.detail_item_idx >= c.checklist.len() && !c.checklist.is_empty() {
                                    b.detail_item_idx = c.checklist.len() - 1;
                                }
                            }
                    }
                }
        }

        // Edit checklist item text
        (KeyCode::Enter, _) => {
            if let Some(board) = &app.board
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

        // Edit description
        (KeyCode::Char('e'), _) => {
            if let Some(board) = &app.board
                && let Some(card) = board.current_card() {
                    let desc = card.description.clone();
                    let card_id = card.id.clone();
                    app.start_insert(Box::new(MarkdownEditor::new(card_id, &desc)));
                }
        }

        // Edit title
        (KeyCode::Char('t'), _) => {
            if let Some(board) = &app.board
                && let Some(card) = board.current_card() {
                    let title = card.title.clone();
                    let card_id = card.id.clone();
                    app.start_insert(Box::new(line_editor::EditCardTitle::new(
                        card_id, &title, false,
                    )));
                }
        }

        // Copy description
        (KeyCode::Char('y'), _) => {
            if let Some(board) = &app.board
                && let Some(card) = board.current_card() {
                    let desc = card.description.clone();
                    app.copy_to_clipboard(desc);
                }
        }

        // Copy entire checklist as markdown
        (KeyCode::Char('Y'), _) => {
            if let Some(board) = &app.board
                && let Some(card) = board.current_card() {
                    if card.checklist.is_empty() {
                        app.set_status("Checklist is empty".into());
                    } else {
                        let md = card.checklist_as_markdown();
                        app.copy_to_clipboard(md);
                    }
                }
        }

        // Labels
        (KeyCode::Char('l'), _) => {
            if let Some(board) = &app.board
                && board.current_card_id().is_some() {
                    app.open_dialog(Box::new(LabelPicker { selected_idx: 0 }));
                }
        }
        (KeyCode::Char('L'), _) => {
            app.open_dialog(Box::new(LabelManager { selected_idx: 0 }));
        }

        // Due date
        (KeyCode::Char('u'), _) => {
            if let Some(board) = &app.board
                && let Some(card) = board.current_card() {
                    let date_str = card
                        .due_date
                        .map(|d| d.format("%Y-%m-%d").to_string())
                        .unwrap_or_default();
                    let card_id = card.id.clone();
                    app.start_insert(Box::new(DatePicker::new(card_id, &date_str)));
                }
        }

        // Clear due date
        (KeyCode::Char('U'), _) => {
            if let Some(board) = &app.board
                && let Some(card_id) = board.current_card_id().cloned() {
                    app.apply(Command::ClearDueDate { card_id })?;
                    app.set_status("Due date cleared".into());
                }
        }

        (KeyCode::Char('h'), _) => {
            if let Some(board) = &app.board
                && board.current_card_id().is_some() {
                    app.open_dialog(Box::new(CardHistory { scroll: 0 }));
                }
        }

        (KeyCode::Char('?'), _) => {
            app.previous_mode = Some(app.mode.clone());
            app.mode = AppMode::Help;
        }
        (KeyCode::Char('q'), _) => {
            app.should_quit = true;
        }

        _ => {}
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
            app.board.as_mut().unwrap().detail_scroll = 5;
            app.board.as_mut().unwrap().detail_item_idx = 2;

            press(&mut app, KeyCode::Esc);

            assert!(matches!(app.mode, AppMode::Normal));
            let board = app.board.as_ref().unwrap();
            assert_eq!(board.detail_scroll, 0);
            assert_eq!(board.detail_item_idx, 0);
        });
    }

    #[test]
    fn down_arrow_navigates_checklist_with_clamp() {
        with_temp_dir(|| {
            let mut app = setup_card_detail();
            press(&mut app, KeyCode::Down);
            assert_eq!(app.board.as_ref().unwrap().detail_item_idx, 1);
            press(&mut app, KeyCode::Down);
            assert_eq!(app.board.as_ref().unwrap().detail_item_idx, 2);
            // Already at last item — clamp
            press(&mut app, KeyCode::Down);
            assert_eq!(app.board.as_ref().unwrap().detail_item_idx, 2);
        });
    }

    #[test]
    fn up_arrow_stops_at_zero() {
        with_temp_dir(|| {
            let mut app = setup_card_detail();
            app.board.as_mut().unwrap().detail_item_idx = 1;
            press(&mut app, KeyCode::Up);
            assert_eq!(app.board.as_ref().unwrap().detail_item_idx, 0);
            press(&mut app, KeyCode::Up);
            assert_eq!(app.board.as_ref().unwrap().detail_item_idx, 0);
        });
    }

    #[test]
    fn space_toggles_current_checklist_item() {
        with_temp_dir(|| {
            let mut app = setup_card_detail();
            press(&mut app, KeyCode::Char(' '));
            let card = app.board.as_ref().unwrap().current_card().unwrap();
            assert!(card.checklist[0].completed);
            press(&mut app, KeyCode::Char(' '));
            let card = app.board.as_ref().unwrap().current_card().unwrap();
            assert!(!card.checklist[0].completed);
        });
    }

    #[test]
    fn x_deletes_current_checklist_item_and_clamps_selection() {
        with_temp_dir(|| {
            let mut app = setup_card_detail();
            app.board.as_mut().unwrap().detail_item_idx = 2; // last
            press(&mut app, KeyCode::Char('x'));
            let board = app.board.as_ref().unwrap();
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
            let board = app.board.as_ref().unwrap();
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
            let card = app.board.as_ref().unwrap().current_card().unwrap();
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
            app.board.as_mut().unwrap().detail_item_idx = 1; // "b"
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
            // Set a multi-line description so scroll is bounded by content.
            let board = app.board.as_mut().unwrap();
            let card_id = board.current_card_id().cloned().unwrap();
            let card = board.cards.get_mut(&card_id).unwrap();
            card.description = (0..30).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
            press(&mut app, KeyCode::PageDown);
            assert!(app.board.as_ref().unwrap().detail_scroll > 0);
            press(&mut app, KeyCode::PageUp);
            assert_eq!(app.board.as_ref().unwrap().detail_scroll, 0);
        });
    }

}

fn scroll_description(app: &mut App, step: usize, down: bool) {
    let accent = app.accent_color();
    let visual_count = if let Some(board) = &app.board {
        if let Some(card) = board.current_card() {
            if card.description.is_empty() {
                1
            } else {
                crate::ui::markdown::highlight_lines(&card.description, accent).len()
            }
        } else {
            0
        }
    } else {
        0
    };
    let max_scroll = visual_count.saturating_sub(1);
    if let Some(board) = &mut app.board {
        if down {
            board.detail_scroll = (board.detail_scroll + step).min(max_scroll);
        } else {
            board.detail_scroll = board.detail_scroll.saturating_sub(step);
        }
    }
}
