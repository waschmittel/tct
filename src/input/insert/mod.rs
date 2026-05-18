//! `Insert` mode dispatch and plain single-line text-buffer editing.
//!
//! `Insert` is parameterized by [`InsertTarget`] (the thing being edited:
//! card title, list name, label, due date, description, …). Specialized
//! targets delegate to submodules:
//!
//! - [`description`] — multi-line markdown description editor.
//! - [`due_date`] — calendar picker for due dates.
//!
//! All other targets share the single-line buffer in [`App::input_buffer`]
//! and are handled directly by [`handle`] / [`confirm_insert`].

mod description;
mod due_date;
mod list_editing;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, AppMode, DialogKind, InsertTarget};
use crate::model::card::Card;
use crate::model::label::Label;
use crate::model::list::CardList;
use crate::storage::{board_store, card_store, list_store};

/// Returns true if either Ctrl or macOS Cmd (Super) is held.
pub(super) fn has_ctrl_or_cmd(modifiers: KeyModifiers) -> bool {
    modifiers.contains(KeyModifiers::CONTROL) || modifiers.contains(KeyModifiers::SUPER)
}

pub fn handle(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    if matches!(app.mode, AppMode::Insert(InsertTarget::EditCardDescription)) {
        return description::handle(app, key);
    }

    if matches!(app.mode, AppMode::Insert(InsertTarget::EditDueDate)) {
        return due_date::handle(app, key);
    }

    match key.code {
        KeyCode::Esc => {
            cancel_insert(app);
        }
        KeyCode::Enter => {
            confirm_insert(app)?;
        }
        KeyCode::Backspace => {
            if app.input_cursor > 0
                && let Some((idx, _)) = app.input_buffer[..app.input_cursor].char_indices().last()
            {
                app.input_buffer.remove(idx);
                app.input_cursor = idx;
            }
        }
        KeyCode::Delete if app.input_cursor < app.input_buffer.len() => {
            app.input_buffer.remove(app.input_cursor);
        }
        KeyCode::Left => {
            if app.input_cursor > 0
                && let Some((idx, _)) = app.input_buffer[..app.input_cursor].char_indices().last()
            {
                app.input_cursor = idx;
            }
        }
        KeyCode::Right => {
            if app.input_cursor < app.input_buffer.len()
                && let Some(c) = app.input_buffer[app.input_cursor..].chars().next()
            {
                app.input_cursor += c.len_utf8();
            }
        }
        KeyCode::Home => {
            app.input_cursor = 0;
        }
        KeyCode::End => {
            app.input_cursor = app.input_buffer.len();
        }
        KeyCode::Char('u') if has_ctrl_or_cmd(key.modifiers) => {
            app.input_buffer.clear();
            app.input_cursor = 0;
        }
        KeyCode::Char('a') if has_ctrl_or_cmd(key.modifiers) => {
            app.input_cursor = 0;
        }
        KeyCode::Char('e') if has_ctrl_or_cmd(key.modifiers) => {
            app.input_cursor = app.input_buffer.len();
        }
        KeyCode::Char(c) => {
            app.input_buffer.insert(app.input_cursor, c);
            app.input_cursor += c.len_utf8();
        }
        _ => {}
    }
    Ok(())
}

/// Leave Insert mode, returning to the most appropriate previous mode.
pub(super) fn cancel_insert(app: &mut App) {
    if let Some(prev) = app.previous_mode.take() {
        app.mode = prev;
    } else {
        let target = match &app.mode {
            AppMode::Insert(t) => t.clone(),
            _ => return,
        };
        app.mode = match target {
            InsertTarget::NewBoardName | InsertTarget::RenameBoard => AppMode::BoardSelector,
            InsertTarget::NewCardTitle
            | InsertTarget::NewListName
            | InsertTarget::RenameList
            | InsertTarget::EditCardTitleInline => AppMode::Normal,
            InsertTarget::EditCardTitle
            | InsertTarget::EditCardDescription
            | InsertTarget::NewChecklistItem
            | InsertTarget::EditChecklistItem
            | InsertTarget::EditDueDate
            | InsertTarget::NewLabelName
            | InsertTarget::EditLabelName => AppMode::CardDetail,
        };
    }
}

fn confirm_insert(app: &mut App) -> anyhow::Result<()> {
    let target = match &app.mode {
        AppMode::Insert(t) => t.clone(),
        _ => return Ok(()),
    };

    let text = app.input_buffer.trim().to_string();

    if text.is_empty() {
        cancel_insert(app);
        return Ok(());
    }

    match target {
        InsertTarget::NewBoardName => {
            let existing_colors: Vec<_> = app.boards.iter().map(|b| b.accent_color).collect();
            let mut meta = crate::model::board::BoardMeta::new(text.clone());
            meta.accent_color = crate::model::label::LabelColor::generate_pastel(&existing_colors);
            board_store::save_board(&meta)?;
            board_store::append_to_order(&meta.id)?;
            app.reload_boards()?;
            app.set_status(format!("Created board '{text}'"));
            app.mode = AppMode::BoardSelector;
        }
        InsertTarget::RenameBoard => {
            if let Some(board) = app.boards.get_mut(app.selected_board_idx) {
                board.name = text.clone();
                board_store::save_board(board)?;
                app.set_status(format!("Renamed board to '{text}'"));
            }
            app.mode = AppMode::BoardSelector;
        }
        InsertTarget::NewCardTitle => {
            if let Some(board) = &mut app.board
                && let Some(list) = board.lists.get_mut(board.selected_list)
            {
                let mut card = Card::new(text.clone());
                card.log("Created");
                card_store::save_card(&board.meta.id, &card)?;
                list.card_ids.push(card.id.clone());
                list_store::save_list(&board.meta.id, list)?;
                board.cards.insert(card.id.clone(), card);
                board.selected_card[board.selected_list] = list.card_ids.len() - 1;
                app.set_status(format!("Added card '{text}'"));
            }
            app.mode = AppMode::Normal;
        }
        InsertTarget::NewListName => {
            if let Some(board) = &mut app.board {
                let list = CardList::new(text.clone());
                list_store::save_list(&board.meta.id, &list)?;
                board.meta.list_order.push(list.id.clone());
                board_store::save_board(&board.meta)?;
                board.lists.push(list);
                board.selected_card.push(0);
                board.scroll_offset.push(0);
                board.selected_list = board.lists.len() - 1;
                app.set_status(format!("Added list '{text}'"));
            }
            app.mode = AppMode::Normal;
        }
        InsertTarget::RenameList => {
            if let Some(board) = &mut app.board
                && let Some(list) = board.lists.get_mut(board.selected_list)
            {
                list.name = text.clone();
                list_store::save_list(&board.meta.id, list)?;
                app.set_status(format!("Renamed list to '{text}'"));
            }
            app.mode = AppMode::Normal;
        }
        InsertTarget::EditCardTitle | InsertTarget::EditCardTitleInline => {
            if let Some(board) = &mut app.board
                && let Some(card_id) = board.current_card_id().cloned()
                && let Some(card) = board.cards.get_mut(&card_id)
            {
                let changed = card.title != text;
                card.title = text;
                if changed {
                    card.log("Edited title");
                } else {
                    card.touch();
                }
                card_store::save_card(&board.meta.id, card)?;
            }
            cancel_insert(app);
        }
        InsertTarget::EditCardDescription => {
            cancel_insert(app);
        }
        InsertTarget::NewChecklistItem => {
            if let Some(board) = &mut app.board
                && let Some(card_id) = board.current_card_id().cloned()
                && let Some(card) = board.cards.get_mut(&card_id)
            {
                let action = format!("Added checklist item '{text}'");
                card.checklist.push(crate::model::card::ChecklistItem {
                    text,
                    completed: false,
                });
                board.detail_item_idx = card.checklist.len() - 1;
                card.log(action);
                card_store::save_card(&board.meta.id, card)?;
            }
            cancel_insert(app);
        }
        InsertTarget::EditChecklistItem => {
            if let Some(board) = &mut app.board
                && let Some(card_id) = board.current_card_id().cloned()
                && let Some(card) = board.cards.get_mut(&card_id)
                && let Some(item) = card.checklist.get_mut(board.detail_item_idx)
            {
                let changed = item.text != text;
                let old = std::mem::replace(&mut item.text, text);
                let new = item.text.clone();
                if changed {
                    card.log(format!("Renamed checklist item '{old}' → '{new}'"));
                } else {
                    card.touch();
                }
                card_store::save_card(&board.meta.id, card)?;
            }
            cancel_insert(app);
        }
        InsertTarget::EditDueDate => {
            // The picker UI confirms via the calendar handler; this path
            // is used only if the user somehow lands here with a plain
            // buffer (e.g. via a legacy code path). Keep the parse-or-clear
            // semantics matching the picker.
            if text.is_empty() || text == "none" {
                if let Some(board) = &mut app.board
                    && let Some(card_id) = board.current_card_id().cloned()
                    && let Some(card) = board.cards.get_mut(&card_id)
                {
                    let was_set = card.due_date.is_some();
                    card.due_date = None;
                    if was_set {
                        card.log("Cleared due date");
                    } else {
                        card.touch();
                    }
                    card_store::save_card(&board.meta.id, card)?;
                }
                app.set_status("Cleared due date".into());
                cancel_insert(app);
            } else if let Ok(date) = chrono::NaiveDate::parse_from_str(&text, "%Y-%m-%d") {
                if let Some(board) = &mut app.board
                    && let Some(card_id) = board.current_card_id().cloned()
                    && let Some(card) = board.cards.get_mut(&card_id)
                {
                    let prev = card.due_date;
                    card.due_date = Some(date);
                    if prev != Some(date) {
                        card.log(format!("Set due date to {date}"));
                    } else {
                        card.touch();
                    }
                    card_store::save_card(&board.meta.id, card)?;
                }
                app.set_status(format!("Due date set to {date}"));
                cancel_insert(app);
            } else {
                app.set_status("Invalid date format. Use YYYY-MM-DD".into());
            }
        }
        InsertTarget::NewLabelName => {
            if let Some(board) = &mut app.board {
                let existing_colors: Vec<_> =
                    board.meta.labels.iter().map(|l| l.color).collect();
                let color = crate::model::label::LabelColor::generate_pastel(&existing_colors);
                let label = Label::new(text.clone(), color);
                board.meta.labels.push(label);
                board_store::save_board(&board.meta)?;
                app.label_picker_idx = board.meta.labels.len().saturating_sub(1);
                app.set_status(format!("Created label '{text}'"));
            }
            app.mode = AppMode::Dialog(DialogKind::LabelManager);
        }
        InsertTarget::EditLabelName => {
            if let Some(board) = &mut app.board
                && let Some(label) = board.meta.labels.get_mut(app.label_picker_idx)
            {
                label.name = text;
                board_store::save_board(&board.meta)?;
            }
            app.mode = AppMode::Dialog(DialogKind::LabelManager);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, AppMode, InsertTarget};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn test_umlaut_insertion_panic() {
        let mut app = App::new(None).unwrap();
        app.mode = AppMode::Insert(InsertTarget::NewCardTitle);
        app.input_buffer.clear();
        app.input_cursor = 0;

        // Insert an umlaut 'ä' (2 bytes in UTF-8: 0xC3 0xA4).
        handle(
            &mut app,
            KeyEvent::new(KeyCode::Char('ä'), KeyModifiers::empty()),
        )
        .unwrap();

        // If the bug exists, app.input_cursor will be 1 — NOT a char
        // boundary — and the next insertion will panic.
        handle(
            &mut app,
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::empty()),
        )
        .unwrap();

        assert_eq!(app.input_buffer, "äb");
        assert_eq!(app.input_cursor, 3); // 2 bytes for 'ä' + 1 byte for 'b'
    }

    #[test]
    fn test_utf8_navigation_and_deletion() {
        let mut app = App::new(None).unwrap();
        app.mode = AppMode::Insert(InsertTarget::NewCardTitle);
        app.input_buffer = "äöü".to_string();
        app.input_cursor = app.input_buffer.len(); // at the end, 6 bytes

        // Backspace once: removes 'ü' (2 bytes)
        handle(
            &mut app,
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()),
        )
        .unwrap();
        assert_eq!(app.input_buffer, "äö");
        assert_eq!(app.input_cursor, 4);

        // Move left: cursor moves from 4 to 2 (pointing at 'ö')
        handle(
            &mut app,
            KeyEvent::new(KeyCode::Left, KeyModifiers::empty()),
        )
        .unwrap();
        assert_eq!(app.input_cursor, 2);

        // Delete: removes 'ö'
        handle(
            &mut app,
            KeyEvent::new(KeyCode::Delete, KeyModifiers::empty()),
        )
        .unwrap();
        assert_eq!(app.input_buffer, "ä");
        assert_eq!(app.input_cursor, 2);

        // Move left: cursor moves from 2 to 0
        handle(
            &mut app,
            KeyEvent::new(KeyCode::Left, KeyModifiers::empty()),
        )
        .unwrap();
        assert_eq!(app.input_cursor, 0);

        // Move right: cursor moves from 0 to 2
        handle(
            &mut app,
            KeyEvent::new(KeyCode::Right, KeyModifiers::empty()),
        )
        .unwrap();
        assert_eq!(app.input_cursor, 2);

        // Backspace: removes 'ä'
        handle(
            &mut app,
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()),
        )
        .unwrap();
        assert_eq!(app.input_buffer, "");
        assert_eq!(app.input_cursor, 0);
    }
}

#[cfg(test)]
mod editor_tests {
    use super::*;
    use crate::app::App;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn editor_lines(app: &App) -> Vec<String> {
        app.description_editor
            .as_ref()
            .unwrap()
            .lines()
            .to_vec()
    }

    fn editor_cursor(app: &App) -> (usize, usize) {
        let ratatui_textarea::DataCursor(r, c) =
            app.description_editor.as_ref().unwrap().cursor();
        (r, c)
    }

    #[test]
    fn enter_at_col0_of_first_list_line_inserts_blank_line_above() {
        let mut app = App::new(None).unwrap();
        app.start_description_edit("- first\n- second");
        // Position cursor at (0, 0) — start of first list item
        let ta = app.description_editor.as_mut().unwrap();
        ta.move_cursor(ratatui_textarea::CursorMove::Top);
        ta.move_cursor(ratatui_textarea::CursorMove::Head);
        assert_eq!(editor_cursor(&app), (0, 0));

        handle(
            &mut app,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
        )
        .unwrap();

        // Blank line inserted above; original list items preserved verbatim
        assert_eq!(
            editor_lines(&app),
            vec![
                "".to_string(),
                "- first".to_string(),
                "- second".to_string()
            ]
        );
        // Cursor on the new blank line so user can type immediately
        assert_eq!(editor_cursor(&app), (0, 0));
    }

    #[test]
    fn enter_at_col0_of_inner_list_line_still_continues_list() {
        let mut app = App::new(None).unwrap();
        app.start_description_edit("intro\n- first\n- second");
        let ta = app.description_editor.as_mut().unwrap();
        ta.move_cursor(ratatui_textarea::CursorMove::Top);
        ta.move_cursor(ratatui_textarea::CursorMove::Down);
        ta.move_cursor(ratatui_textarea::CursorMove::Head);
        assert_eq!(editor_cursor(&app), (1, 0));

        handle(
            &mut app,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
        )
        .unwrap();

        assert_eq!(
            editor_lines(&app),
            vec![
                "intro".to_string(),
                "- first".to_string(),
                "- ".to_string(),
                "- second".to_string()
            ]
        );
    }

    #[test]
    fn enter_at_end_of_list_item_continues_list() {
        let mut app = App::new(None).unwrap();
        app.start_description_edit("- first");
        let ta = app.description_editor.as_mut().unwrap();
        ta.move_cursor(ratatui_textarea::CursorMove::End);

        handle(
            &mut app,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
        )
        .unwrap();

        assert_eq!(
            editor_lines(&app),
            vec!["- first".to_string(), "- ".to_string()]
        );
    }

    #[test]
    fn enter_at_end_of_numbered_item_continues_with_next_number() {
        let mut app = App::new(None).unwrap();
        app.start_description_edit("1. first");
        let ta = app.description_editor.as_mut().unwrap();
        ta.move_cursor(ratatui_textarea::CursorMove::End);

        handle(
            &mut app,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
        )
        .unwrap();

        assert_eq!(
            editor_lines(&app),
            vec!["1. first".to_string(), "2. ".to_string()]
        );
    }

    #[test]
    fn enter_in_numbered_list_renumbers_following_items() {
        let mut app = App::new(None).unwrap();
        app.start_description_edit("1. a\n2. b\n3. c");
        let ta = app.description_editor.as_mut().unwrap();
        ta.move_cursor(ratatui_textarea::CursorMove::Top);
        ta.move_cursor(ratatui_textarea::CursorMove::End);

        handle(
            &mut app,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
        )
        .unwrap();

        assert_eq!(
            editor_lines(&app),
            vec![
                "1. a".to_string(),
                "2. ".to_string(),
                "3. b".to_string(),
                "4. c".to_string(),
            ]
        );
        assert_eq!(editor_cursor(&app), (1, 3));
    }

    #[test]
    fn enter_at_last_numbered_item_does_not_renumber() {
        let mut app = App::new(None).unwrap();
        app.start_description_edit("1. a\n2. b");
        let ta = app.description_editor.as_mut().unwrap();
        ta.move_cursor(ratatui_textarea::CursorMove::Bottom);
        ta.move_cursor(ratatui_textarea::CursorMove::End);

        handle(
            &mut app,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
        )
        .unwrap();

        assert_eq!(
            editor_lines(&app),
            vec!["1. a".to_string(), "2. b".to_string(), "3. ".to_string()]
        );
    }

    #[test]
    fn enter_in_parent_numbered_list_skips_nested_children() {
        let mut app = App::new(None).unwrap();
        app.start_description_edit("1. parent\n   1. child\n   2. child\n2. parent2");
        let ta = app.description_editor.as_mut().unwrap();
        ta.move_cursor(ratatui_textarea::CursorMove::Top);
        ta.move_cursor(ratatui_textarea::CursorMove::End);

        handle(
            &mut app,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
        )
        .unwrap();

        assert_eq!(
            editor_lines(&app),
            vec![
                "1. parent".to_string(),
                "2. ".to_string(),
                "   1. child".to_string(),
                "   2. child".to_string(),
                "3. parent2".to_string(),
            ]
        );
    }

    #[test]
    fn enter_in_nested_numbered_list_does_not_renumber_parents() {
        let mut app = App::new(None).unwrap();
        app.start_description_edit("1. parent\n   1. child\n   2. child2\n2. parent2");
        let ta = app.description_editor.as_mut().unwrap();
        ta.move_cursor(ratatui_textarea::CursorMove::Top);
        ta.move_cursor(ratatui_textarea::CursorMove::Down);
        ta.move_cursor(ratatui_textarea::CursorMove::End);

        handle(
            &mut app,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
        )
        .unwrap();

        assert_eq!(
            editor_lines(&app),
            vec![
                "1. parent".to_string(),
                "   1. child".to_string(),
                "   2. ".to_string(),
                "   3. child2".to_string(),
                "2. parent2".to_string(),
            ]
        );
    }

    #[test]
    fn enter_renumbers_non_canonical_numbered_list_fully() {
        let mut app = App::new(None).unwrap();
        app.start_description_edit("1. a\n5. b\n7. c");
        let ta = app.description_editor.as_mut().unwrap();
        ta.move_cursor(ratatui_textarea::CursorMove::Top);
        ta.move_cursor(ratatui_textarea::CursorMove::End);

        handle(
            &mut app,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
        )
        .unwrap();

        assert_eq!(
            editor_lines(&app),
            vec![
                "1. a".to_string(),
                "2. ".to_string(),
                "3. b".to_string(),
                "4. c".to_string(),
            ]
        );
        assert_eq!(editor_cursor(&app), (1, 3));
    }

    #[test]
    fn enter_preserves_lists_starting_at_nonzero_number() {
        let mut app = App::new(None).unwrap();
        app.start_description_edit("3. a\n4. b\n4. c");
        let ta = app.description_editor.as_mut().unwrap();
        ta.move_cursor(ratatui_textarea::CursorMove::Top);
        ta.move_cursor(ratatui_textarea::CursorMove::End);

        handle(
            &mut app,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
        )
        .unwrap();

        assert_eq!(
            editor_lines(&app),
            vec![
                "3. a".to_string(),
                "4. ".to_string(),
                "5. b".to_string(),
                "6. c".to_string(),
            ]
        );
    }

    #[test]
    fn enter_in_middle_of_list_renumbers_items_above_and_below() {
        let mut app = App::new(None).unwrap();
        app.start_description_edit("1. a\n1. b\n1. c\n1. d");
        let ta = app.description_editor.as_mut().unwrap();
        ta.move_cursor(ratatui_textarea::CursorMove::Top);
        ta.move_cursor(ratatui_textarea::CursorMove::Down);
        ta.move_cursor(ratatui_textarea::CursorMove::End);

        handle(
            &mut app,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
        )
        .unwrap();

        assert_eq!(
            editor_lines(&app),
            vec![
                "1. a".to_string(),
                "2. b".to_string(),
                "3. ".to_string(),
                "4. c".to_string(),
                "5. d".to_string(),
            ]
        );
    }

    #[test]
    fn enter_below_paragraph_renumbers_only_the_list_run() {
        let mut app = App::new(None).unwrap();
        app.start_description_edit("intro text\n1. foo\n2. bar");
        let ta = app.description_editor.as_mut().unwrap();
        ta.move_cursor(ratatui_textarea::CursorMove::Top);
        ta.move_cursor(ratatui_textarea::CursorMove::Down);
        ta.move_cursor(ratatui_textarea::CursorMove::End);

        handle(
            &mut app,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
        )
        .unwrap();

        assert_eq!(
            editor_lines(&app),
            vec![
                "intro text".to_string(),
                "1. foo".to_string(),
                "2. ".to_string(),
                "3. bar".to_string(),
            ]
        );
    }

    #[test]
    fn enter_does_not_touch_earlier_unrelated_numbered_list() {
        let mut app = App::new(None).unwrap();
        app.start_description_edit("1. orphan\nunrelated\n1. foo\n2. bar");
        let ta = app.description_editor.as_mut().unwrap();
        ta.move_cursor(ratatui_textarea::CursorMove::Top);
        ta.move_cursor(ratatui_textarea::CursorMove::Down);
        ta.move_cursor(ratatui_textarea::CursorMove::Down);
        ta.move_cursor(ratatui_textarea::CursorMove::End);

        handle(
            &mut app,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
        )
        .unwrap();

        assert_eq!(
            editor_lines(&app),
            vec![
                "1. orphan".to_string(),
                "unrelated".to_string(),
                "1. foo".to_string(),
                "2. ".to_string(),
                "3. bar".to_string(),
            ]
        );
    }

    #[test]
    fn tab_nests_numbered_list_item_and_resets_to_one() {
        let mut app = App::new(None).unwrap();
        app.start_description_edit("1. parent\n2. child");
        let ta = app.description_editor.as_mut().unwrap();
        ta.move_cursor(ratatui_textarea::CursorMove::Bottom);
        ta.move_cursor(ratatui_textarea::CursorMove::End);

        handle(
            &mut app,
            KeyEvent::new(KeyCode::Tab, KeyModifiers::empty()),
        )
        .unwrap();

        assert_eq!(
            editor_lines(&app),
            vec!["1. parent".to_string(), "   1. child".to_string()]
        );
    }

    #[test]
    fn tab_nests_into_existing_nested_run() {
        let mut app = App::new(None).unwrap();
        app.start_description_edit("1. parent\n   1. existing-nested\n2. orphan");
        let ta = app.description_editor.as_mut().unwrap();
        ta.move_cursor(ratatui_textarea::CursorMove::Bottom);
        ta.move_cursor(ratatui_textarea::CursorMove::End);

        handle(
            &mut app,
            KeyEvent::new(KeyCode::Tab, KeyModifiers::empty()),
        )
        .unwrap();

        assert_eq!(
            editor_lines(&app),
            vec![
                "1. parent".to_string(),
                "   1. existing-nested".to_string(),
                "   2. orphan".to_string(),
            ]
        );
    }

    #[test]
    fn tab_nests_unordered_list_item() {
        let mut app = App::new(None).unwrap();
        app.start_description_edit("- a\n- b");
        let ta = app.description_editor.as_mut().unwrap();
        ta.move_cursor(ratatui_textarea::CursorMove::Bottom);
        ta.move_cursor(ratatui_textarea::CursorMove::End);

        handle(
            &mut app,
            KeyEvent::new(KeyCode::Tab, KeyModifiers::empty()),
        )
        .unwrap();

        assert_eq!(
            editor_lines(&app),
            vec!["- a".to_string(), "   - b".to_string()]
        );
    }

    #[test]
    fn shift_tab_unnests_nested_item_and_joins_parent_list() {
        let mut app = App::new(None).unwrap();
        app.start_description_edit("1. parent\n   1. child");
        let ta = app.description_editor.as_mut().unwrap();
        ta.move_cursor(ratatui_textarea::CursorMove::Bottom);
        ta.move_cursor(ratatui_textarea::CursorMove::End);

        handle(
            &mut app,
            KeyEvent::new(KeyCode::BackTab, KeyModifiers::empty()),
        )
        .unwrap();

        assert_eq!(
            editor_lines(&app),
            vec!["1. parent".to_string(), "2. child".to_string()]
        );
    }

    #[test]
    fn shift_tab_on_top_level_list_item_is_noop() {
        let mut app = App::new(None).unwrap();
        app.start_description_edit("1. a\n2. b");
        let ta = app.description_editor.as_mut().unwrap();
        ta.move_cursor(ratatui_textarea::CursorMove::Bottom);
        ta.move_cursor(ratatui_textarea::CursorMove::End);

        handle(
            &mut app,
            KeyEvent::new(KeyCode::BackTab, KeyModifiers::empty()),
        )
        .unwrap();

        assert_eq!(
            editor_lines(&app),
            vec!["1. a".to_string(), "2. b".to_string()]
        );
    }

    #[test]
    fn deleting_blank_line_between_list_items_renumbers() {
        let mut app = App::new(None).unwrap();
        app.start_description_edit("1. a\n\n5. b\n7. c");
        let ta = app.description_editor.as_mut().unwrap();
        ta.move_cursor(ratatui_textarea::CursorMove::Top);
        ta.move_cursor(ratatui_textarea::CursorMove::Down);
        ta.move_cursor(ratatui_textarea::CursorMove::Head);

        handle(
            &mut app,
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()),
        )
        .unwrap();

        assert_eq!(
            editor_lines(&app),
            vec!["1. a".to_string(), "2. b".to_string(), "3. c".to_string()]
        );
    }

    #[test]
    fn deleting_numbered_item_via_line_merge_renumbers() {
        let mut app = App::new(None).unwrap();
        app.start_description_edit("1. a\n2. b\n3. c");
        let ta = app.description_editor.as_mut().unwrap();
        ta.move_cursor(ratatui_textarea::CursorMove::Top);
        ta.move_cursor(ratatui_textarea::CursorMove::Down);
        ta.move_cursor(ratatui_textarea::CursorMove::Head);

        handle(
            &mut app,
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()),
        )
        .unwrap();

        // Backspace at start of "2. b" merges it into the previous line:
        // line 0 becomes "1. a2. b" — still a "1." numbered prefix, so the
        // run continues. The trailing "3. c" gets renumbered to "2. c".
        assert_eq!(
            editor_lines(&app),
            vec!["1. a2. b".to_string(), "2. c".to_string()]
        );
    }

    #[test]
    fn typing_does_not_trigger_renumber() {
        let mut app = App::new(None).unwrap();
        app.start_description_edit("1. a\n5. b");
        let ta = app.description_editor.as_mut().unwrap();
        ta.move_cursor(ratatui_textarea::CursorMove::Top);
        ta.move_cursor(ratatui_textarea::CursorMove::End);

        // Type a character — must not rewrite "5. b" to "2. b".
        handle(
            &mut app,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::empty()),
        )
        .unwrap();

        assert_eq!(
            editor_lines(&app),
            vec!["1. ax".to_string(), "5. b".to_string()]
        );
    }
}
