use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui_textarea::CursorMove;

use crate::app::{App, AppMode, DialogKind, InsertTarget};
use crate::model::card::Card;
use crate::model::label::Label;
use crate::model::list::CardList;
use crate::storage::{board_store, card_store, list_store};

fn has_ctrl_or_cmd(modifiers: KeyModifiers) -> bool {
    modifiers.contains(KeyModifiers::CONTROL) || modifiers.contains(KeyModifiers::SUPER)
}

pub fn handle(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    if matches!(
        app.mode,
        AppMode::Insert(InsertTarget::EditCardDescription)
    ) {
        return handle_description_edit(app, key);
    }

    match key.code {
        KeyCode::Esc => {
            cancel_insert(app);
        }
        KeyCode::Enter => {
            confirm_insert(app)?;
        }
        KeyCode::Backspace => {
            if app.input_cursor > 0 {
                if let Some((idx, _)) = app.input_buffer[..app.input_cursor].char_indices().last() {
                    app.input_buffer.remove(idx);
                    app.input_cursor = idx;
                }
            }
        }
        KeyCode::Delete => {
            if app.input_cursor < app.input_buffer.len() {
                app.input_buffer.remove(app.input_cursor);
            }
        }
        KeyCode::Left => {
            if app.input_cursor > 0 {
                if let Some((idx, _)) = app.input_buffer[..app.input_cursor].char_indices().last() {
                    app.input_cursor = idx;
                }
            }
        }
        KeyCode::Right => {
            if app.input_cursor < app.input_buffer.len() {
                if let Some(c) = app.input_buffer[app.input_cursor..].chars().next() {
                    app.input_cursor += c.len_utf8();
                }
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

fn handle_description_edit(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('s'), m) if has_ctrl_or_cmd(m) => {
            confirm_description_save(app)?;
        }
        (KeyCode::Esc, _) => {
            let changed = description_changed(app);
            if changed {
                app.mode = AppMode::Dialog(DialogKind::ConfirmCancelEdit);
            } else {
                app.description_editor = None;
                app.description_original = None;
                app.mode = app.previous_mode.take().unwrap_or(AppMode::CardDetail);
            }
        }
        (KeyCode::Char('z'), m) if has_ctrl_or_cmd(m) => {
            if let Some(textarea) = &mut app.description_editor {
                textarea.undo();
            }
        }
        (KeyCode::Char('y'), m) if has_ctrl_or_cmd(m) => {
            if let Some(textarea) = &mut app.description_editor {
                textarea.redo();
            }
        }
        (KeyCode::Char('b'), m) if has_ctrl_or_cmd(m) => {
            wrap_selection_or_insert(app, "**", "**");
        }
        (KeyCode::Char('i'), m) if has_ctrl_or_cmd(m) => {
            wrap_selection_or_insert(app, "*", "*");
        }
        (KeyCode::Char('k'), m) if has_ctrl_or_cmd(m) => {
            wrap_selection_or_insert(app, "`", "`");
        }
        (KeyCode::Char('l'), m) if has_ctrl_or_cmd(m) => {
            insert_at_line_start(app, "- ");
        }
        (KeyCode::Enter, _) => {
            handle_enter_in_list(app);
        }
        (KeyCode::Tab, m) if !m.contains(KeyModifiers::SHIFT) => {
            if !handle_tab_nest(app) {
                if let Some(textarea) = &mut app.description_editor {
                    textarea.input(key);
                }
            }
        }
        (KeyCode::BackTab, _) | (KeyCode::Tab, _) => {
            handle_shift_tab_unnest(app);
        }
        (KeyCode::Up, _) => {
            move_cursor_visual(app, -1);
        }
        (KeyCode::Down, _) => {
            move_cursor_visual(app, 1);
        }
        _ => {
            if let Some(textarea) = &mut app.description_editor {
                // Renumber when an edit changes the document's line count
                // (Backspace joining lines, Delete merging, paste of multi-
                // line text). Plain char-by-char edits leave the count
                // unchanged and skip the renumber pass.
                let before = textarea.lines().len();
                textarea.input(key);
                let after = textarea.lines().len();
                if after != before {
                    renumber_all(textarea);
                }
            }
        }
    }
    update_editor_scroll(app);
    Ok(())
}

fn update_editor_scroll(app: &mut App) {
    if let Some(textarea) = &app.description_editor {
        let ratatui_textarea::DataCursor(cursor_row, _) = textarea.cursor();
        let visible_height = 20usize;
        if cursor_row < app.editor_scroll {
            app.editor_scroll = cursor_row;
        } else if cursor_row >= app.editor_scroll + visible_height {
            app.editor_scroll = cursor_row - visible_height + 1;
        }
    }
}

fn move_cursor_visual(app: &mut App, direction: i32) {
    use crate::ui::markdown;

    let accent = app.accent_color();
    let Some(textarea) = &app.description_editor else {
        return;
    };
    let ratatui_textarea::DataCursor(cursor_row, cursor_col) = textarea.cursor();
    let lines: Vec<String> = textarea.lines().to_vec();

    let visual_map = markdown::build_visual_map(&lines, accent, markdown::WRAP_WIDTH);
    let (current_vrow, visual_col) = markdown::source_to_visual(&visual_map, cursor_row, cursor_col);

    let target_vrow = if direction < 0 {
        current_vrow.checked_sub(1)
    } else {
        let next = current_vrow + 1;
        if next < visual_map.len() {
            Some(next)
        } else {
            None
        }
    };

    let Some(target_vrow) = target_vrow else {
        return;
    };

    let (target_src_row, target_src_offset, target_vlen, target_vindent) = visual_map[target_vrow];
    let actual_target_vlen = target_vlen.saturating_sub(target_vindent);
    let target_col = target_src_offset + (visual_col.saturating_sub(target_vindent)).min(actual_target_vlen);

    let textarea = app.description_editor.as_mut().unwrap();
    textarea.move_cursor(CursorMove::Jump(target_src_row as u16, target_col as u16));
}

fn description_changed(app: &App) -> bool {
    let current = app
        .description_editor
        .as_ref()
        .map(|ta| ta.lines().join("\n"))
        .unwrap_or_default();
    let original = app.description_original.as_deref().unwrap_or("");
    current != original
}

fn confirm_description_save(app: &mut App) -> anyhow::Result<()> {
    let text = app.finish_description_edit().unwrap_or_default();
    if let Some(board) = &mut app.board {
        if let Some(card_id) = board.current_card_id().cloned() {
            if let Some(card) = board.cards.get_mut(&card_id) {
                card.description = text;
                card.touch();
                card_store::save_card(&board.meta.id, card)?;
                app.set_status("Description saved".into());
            }
        }
    }
    app.description_original = None;
    app.mode = app.previous_mode.take().unwrap_or(AppMode::CardDetail);
    Ok(())
}

fn handle_enter_in_list(app: &mut App) {
    let Some(textarea) = &mut app.description_editor else {
        return;
    };
    let ratatui_textarea::DataCursor(row, col) = textarea.cursor();

    // At the very start of the document: insert a blank line above without
    // list continuation, so users can add content before a leading list item.
    // Only special-cased for (0, 0) — for other positions, users can navigate
    // to the previous line and continue normally.
    if row == 0 && col == 0 {
        textarea.insert_newline();
        textarea.move_cursor(CursorMove::Up);
        return;
    }

    let current_line = textarea.lines().get(row).cloned().unwrap_or_default();
    let trimmed = current_line.trim_start();

    if trimmed == "-" || trimmed == "*" || trimmed == "- " || trimmed == "* " {
        textarea.move_cursor(CursorMove::Head);
        textarea.delete_line_by_end();
        textarea.insert_newline();
        return;
    }
    if let Some(num_str) = trimmed.strip_suffix(". ").or_else(|| trimmed.strip_suffix('.')) {
        if num_str.parse::<u64>().is_ok() {
            textarea.move_cursor(CursorMove::Head);
            textarea.delete_line_by_end();
            textarea.insert_newline();
            return;
        }
    }

    if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
        let indent = current_line.len() - trimmed.len();
        let prefix_char = &trimmed[..2];
        let indent_str = " ".repeat(indent);
        textarea.move_cursor(CursorMove::End);
        textarea.insert_newline();
        textarea.insert_str(&format!("{indent_str}{prefix_char}"));
        return;
    }

    if let Some(dot_pos) = trimmed.find(". ") {
        let num_part = &trimmed[..dot_pos];
        if num_part.parse::<u64>().is_ok() {
            let indent = current_line.len() - trimmed.len();
            let indent_str = " ".repeat(indent);
            textarea.move_cursor(CursorMove::End);
            textarea.insert_newline();
            // Use a placeholder number (renumber_all rewrites it to the
            // correct value, so the literal here doesn't matter beyond shape).
            textarea.insert_str(&format!("{indent_str}1. "));
            renumber_all(textarea);
            // Park the cursor at the end of the new item, regardless of how
            // many digits its prefix ended up with.
            let new_row = row + 1;
            let new_len = textarea
                .lines()
                .get(new_row)
                .map(|l| l.chars().count())
                .unwrap_or(0);
            textarea.move_cursor(CursorMove::Jump(new_row as u16, new_len as u16));
            return;
        }
    }

    textarea.insert_newline();
}

const NEST_INDENT: usize = 3;

/// Returns true if the cursor was on a list line and the nest was applied.
/// On a numbered list the inserted item is forced to start at 1 so it
/// becomes the start of a fresh nested list (renumber_all then joins it
/// to any existing nested run at the new indent).
fn handle_tab_nest(app: &mut App) -> bool {
    let Some(textarea) = &mut app.description_editor else {
        return false;
    };
    let ratatui_textarea::DataCursor(row, col) = textarea.cursor();
    let line = match textarea.lines().get(row) {
        Some(l) => l.clone(),
        None => return false,
    };
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();

    let numbered_prefix = trimmed
        .find(". ")
        .and_then(|p| trimmed[..p].parse::<u64>().ok().map(|_| p));
    let is_unordered = trimmed.starts_with("- ") || trimmed.starts_with("* ");

    if numbered_prefix.is_none() && !is_unordered {
        return false;
    }

    let pad = " ".repeat(NEST_INDENT);
    textarea.move_cursor(CursorMove::Jump(row as u16, 0));
    textarea.insert_str(&pad);

    if let Some(dot_pos) = numbered_prefix {
        let old_num_str = &trimmed[..dot_pos];
        let new_indent = indent + NEST_INDENT;
        textarea.move_cursor(CursorMove::Jump(row as u16, new_indent as u16));
        textarea.delete_str(old_num_str.chars().count());
        textarea.insert_str("1");
    }

    renumber_all(textarea);

    let new_line_len = textarea
        .lines()
        .get(row)
        .map(|l| l.chars().count())
        .unwrap_or(0);
    let target_col = (col + NEST_INDENT).min(new_line_len);
    textarea.move_cursor(CursorMove::Jump(row as u16, target_col as u16));
    true
}

/// Remove one indent level (`NEST_INDENT` leading spaces) if the cursor
/// is on a list line that has enough leading spaces. After dedenting,
/// renumber_all rejoins the line to the parent-level list.
fn handle_shift_tab_unnest(app: &mut App) {
    let Some(textarea) = &mut app.description_editor else {
        return;
    };
    let ratatui_textarea::DataCursor(row, col) = textarea.cursor();
    let line = match textarea.lines().get(row) {
        Some(l) => l.clone(),
        None => return,
    };
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();
    if indent < NEST_INDENT {
        return;
    }

    let is_numbered = trimmed
        .find(". ")
        .and_then(|p| trimmed[..p].parse::<u64>().ok())
        .is_some();
    let is_unordered = trimmed.starts_with("- ") || trimmed.starts_with("* ");
    if !is_numbered && !is_unordered {
        return;
    }

    textarea.move_cursor(CursorMove::Jump(row as u16, 0));
    textarea.delete_str(NEST_INDENT);

    renumber_all(textarea);

    let new_line_len = textarea
        .lines()
        .get(row)
        .map(|l| l.chars().count())
        .unwrap_or(0);
    let target_col = col.saturating_sub(NEST_INDENT).min(new_line_len);
    textarea.move_cursor(CursorMove::Jump(row as u16, target_col as u16));
}

/// Walk the entire document with a per-indent stack of "next expected
/// number" counters and rewrite every numbered list item so each
/// contiguous run is sequential. Each run's start number is preserved
/// from the run's first item, so lists that intentionally start at a
/// non-1 number keep their starting point. Blank lines reset all
/// active runs; non-numbered lines end runs at indent >= their indent
/// so unrelated lists stay independent. Cursor position is preserved.
fn renumber_all(textarea: &mut ratatui_textarea::TextArea<'static>) {
    let lines = textarea.lines().to_vec();
    let saved = textarea.cursor();

    let mut stack: Vec<(usize, u64)> = Vec::new();

    for (r, line) in lines.iter().enumerate() {
        if line.is_empty() {
            stack.clear();
            continue;
        }
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();

        let numbered = trimmed.find(". ").and_then(|p| {
            let s = &trimmed[..p];
            s.parse::<u64>().ok().map(|n| (s.to_string(), n))
        });

        if let Some((num_str, n)) = numbered {
            while let Some(&(top_indent, _)) = stack.last() {
                if top_indent > indent {
                    stack.pop();
                } else {
                    break;
                }
            }

            let expected = match stack.last() {
                Some(&(top_indent, next)) if top_indent == indent => next,
                _ => n,
            };

            match stack.last_mut() {
                Some(top) if top.0 == indent => top.1 = expected + 1,
                _ => stack.push((indent, expected + 1)),
            }

            if expected != n {
                textarea.move_cursor(CursorMove::Jump(r as u16, indent as u16));
                textarea.delete_str(num_str.chars().count());
                textarea.insert_str(&expected.to_string());
            }
        } else {
            // A non-blank, non-numbered line ends any run whose indent
            // is >= this line's indent. Deeper-indented continuation
            // lines (indent strictly greater than every active level)
            // are left alone.
            while let Some(&(top_indent, _)) = stack.last() {
                if top_indent >= indent {
                    stack.pop();
                } else {
                    break;
                }
            }
        }
    }

    let ratatui_textarea::DataCursor(sr, sc) = saved;
    textarea.move_cursor(CursorMove::Jump(sr as u16, sc as u16));
}

fn wrap_selection_or_insert(app: &mut App, prefix: &str, suffix: &str) {
    let Some(textarea) = &mut app.description_editor else {
        return;
    };
    if textarea.is_selecting() {
        textarea.cut();
        let selected = textarea.yank_text().to_string();
        textarea.insert_str(&format!("{prefix}{selected}{suffix}"));
    } else {
        textarea.insert_str(&format!("{prefix}{suffix}"));
        for _ in 0..suffix.len() {
            textarea.move_cursor(CursorMove::Back);
        }
    }
}

fn insert_at_line_start(app: &mut App, prefix: &str) {
    let Some(textarea) = &mut app.description_editor else {
        return;
    };
    textarea.move_cursor(CursorMove::Head);
    textarea.insert_str(prefix);
}

fn cancel_insert(app: &mut App) {
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
        // ... (implementation of target handlers)
        InsertTarget::NewBoardName => {
            let existing_colors: Vec<_> =
                app.boards.iter().map(|b| b.accent_color).collect();
            let mut meta = crate::model::board::BoardMeta::new(text.clone());
            meta.accent_color =
                crate::model::label::LabelColor::generate_pastel(&existing_colors);
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
            if let Some(board) = &mut app.board {
                if let Some(list) = board.lists.get_mut(board.selected_list) {
                    let card = Card::new(text.clone());
                    card_store::save_card(&board.meta.id, &card)?;
                    list.card_ids.push(card.id.clone());
                    list_store::save_list(&board.meta.id, list)?;
                    board.cards.insert(card.id.clone(), card);
                    board.selected_card[board.selected_list] = list.card_ids.len() - 1;
                    app.set_status(format!("Added card '{text}'"));
                }
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
            if let Some(board) = &mut app.board {
                if let Some(list) = board.lists.get_mut(board.selected_list) {
                    list.name = text.clone();
                    list_store::save_list(&board.meta.id, list)?;
                    app.set_status(format!("Renamed list to '{text}'"));
                }
            }
            app.mode = AppMode::Normal;
        }
        InsertTarget::EditCardTitle | InsertTarget::EditCardTitleInline => {
            if let Some(board) = &mut app.board {
                if let Some(card_id) = board.current_card_id().cloned() {
                    if let Some(card) = board.cards.get_mut(&card_id) {
                        card.title = text;
                        card.touch();
                        card_store::save_card(&board.meta.id, card)?;
                    }
                }
            }
            cancel_insert(app);
        }
        InsertTarget::EditCardDescription => {
            cancel_insert(app);
        }
        InsertTarget::NewChecklistItem => {
            if let Some(board) = &mut app.board {
                if let Some(card_id) = board.current_card_id().cloned() {
                    if let Some(card) = board.cards.get_mut(&card_id) {
                        card.checklist.push(crate::model::card::ChecklistItem {
                            text,
                            completed: false,
                        });
                        board.detail_item_idx = card.checklist.len() - 1;
                        card.touch();
                        card_store::save_card(&board.meta.id, card)?;
                    }
                }
            }
            cancel_insert(app);
        }
        InsertTarget::EditChecklistItem => {
            if let Some(board) = &mut app.board {
                if let Some(card_id) = board.current_card_id().cloned() {
                    if let Some(card) = board.cards.get_mut(&card_id) {
                        if let Some(item) = card.checklist.get_mut(board.detail_item_idx) {
                            item.text = text;
                            card.touch();
                            card_store::save_card(&board.meta.id, card)?;
                        }
                    }
                }
            }
            cancel_insert(app);
        }
        InsertTarget::EditDueDate => {
            if text.is_empty() || text == "none" {
                if let Some(board) = &mut app.board {
                    if let Some(card_id) = board.current_card_id().cloned() {
                        if let Some(card) = board.cards.get_mut(&card_id) {
                            card.due_date = None;
                            card.touch();
                            card_store::save_card(&board.meta.id, card)?;
                        }
                    }
                }
                app.set_status("Cleared due date".into());
                cancel_insert(app);
            } else if let Ok(date) = chrono::NaiveDate::parse_from_str(&text, "%Y-%m-%d") {
                if let Some(board) = &mut app.board {
                    if let Some(card_id) = board.current_card_id().cloned() {
                        if let Some(card) = board.cards.get_mut(&card_id) {
                            card.due_date = Some(date);
                            card.touch();
                            card_store::save_card(&board.meta.id, card)?;
                        }
                    }
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
            if let Some(board) = &mut app.board {
                if let Some(label) = board.meta.labels.get_mut(app.label_picker_idx) {
                    label.name = text;
                    board_store::save_board(&board.meta)?;
                }
            }
            app.mode = AppMode::Dialog(DialogKind::LabelManager);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyCode, KeyModifiers};
    use crate::app::{App, AppMode, InsertTarget};

    #[test]
    fn test_umlaut_insertion_panic() {
        // App::new might fail if directories don't exist, but it calls board_store::ensure_base_dirs()
        // which might fail in a restricted environment. We'll see.
        let mut app = App::new(None).unwrap();
        app.mode = AppMode::Insert(InsertTarget::NewCardTitle);
        app.input_buffer.clear();
        app.input_cursor = 0;

        // Insert an umlaut 'ä' (2 bytes in UTF-8: 0xC3 0xA4)
        handle(&mut app, KeyEvent::new(KeyCode::Char('ä'), KeyModifiers::empty())).unwrap();
        
        // If the bug exists, app.input_cursor will be 1, which is NOT a char boundary.
        // The next insertion will panic.
        handle(&mut app, KeyEvent::new(KeyCode::Char('b'), KeyModifiers::empty())).unwrap();
        
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
        handle(&mut app, KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty())).unwrap();
        assert_eq!(app.input_buffer, "äö");
        assert_eq!(app.input_cursor, 4);

        // Move left: cursor moves from 4 to 2 (pointing at 'ö')
        handle(&mut app, KeyEvent::new(KeyCode::Left, KeyModifiers::empty())).unwrap();
        assert_eq!(app.input_cursor, 2);

        // Delete: removes 'ö'
        handle(&mut app, KeyEvent::new(KeyCode::Delete, KeyModifiers::empty())).unwrap();
        assert_eq!(app.input_buffer, "ä");
        assert_eq!(app.input_cursor, 2);

        // Move left: cursor moves from 2 to 0
        handle(&mut app, KeyEvent::new(KeyCode::Left, KeyModifiers::empty())).unwrap();
        assert_eq!(app.input_cursor, 0);

        // Move right: cursor moves from 0 to 2
        handle(&mut app, KeyEvent::new(KeyCode::Right, KeyModifiers::empty())).unwrap();
        assert_eq!(app.input_cursor, 2);

        // Backspace: removes 'ä'
        handle(&mut app, KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty())).unwrap();
        assert_eq!(app.input_buffer, "");
        assert_eq!(app.input_cursor, 0);
    }

    fn editor_lines(app: &App) -> Vec<String> {
        app.description_editor
            .as_ref()
            .unwrap()
            .lines()
            .to_vec()
    }

    fn editor_cursor(app: &App) -> (usize, usize) {
        let ratatui_textarea::DataCursor(r, c) = app.description_editor.as_ref().unwrap().cursor();
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

        handle(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())).unwrap();

        // Blank line inserted above; original list items preserved verbatim
        assert_eq!(
            editor_lines(&app),
            vec!["".to_string(), "- first".to_string(), "- second".to_string()]
        );
        // Cursor on the new blank line so user can type immediately
        assert_eq!(editor_cursor(&app), (0, 0));
    }

    #[test]
    fn enter_at_col0_of_inner_list_line_still_continues_list() {
        // Regression guard: the col-0 fix is scoped to (0, 0). Inner list lines
        // keep the original auto-continuation behavior.
        let mut app = App::new(None).unwrap();
        app.start_description_edit("intro\n- first\n- second");
        // Position cursor at (1, 0) — start of first list item
        let ta = app.description_editor.as_mut().unwrap();
        ta.move_cursor(ratatui_textarea::CursorMove::Top);
        ta.move_cursor(ratatui_textarea::CursorMove::Down);
        ta.move_cursor(ratatui_textarea::CursorMove::Head);
        assert_eq!(editor_cursor(&app), (1, 0));

        handle(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())).unwrap();

        // Auto-continuation kicks in: new "- " line inserted after the list item
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

        handle(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())).unwrap();

        // Continuation: new line gets "- " bullet prefix
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

        handle(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())).unwrap();

        assert_eq!(
            editor_lines(&app),
            vec!["1. first".to_string(), "2. ".to_string()]
        );
    }

    #[test]
    fn enter_in_numbered_list_renumbers_following_items() {
        let mut app = App::new(None).unwrap();
        app.start_description_edit("1. a\n2. b\n3. c");
        // Cursor at end of "1. a"
        let ta = app.description_editor.as_mut().unwrap();
        ta.move_cursor(ratatui_textarea::CursorMove::Top);
        ta.move_cursor(ratatui_textarea::CursorMove::End);

        handle(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())).unwrap();

        // Following items renumbered to keep the list contiguous
        assert_eq!(
            editor_lines(&app),
            vec![
                "1. a".to_string(),
                "2. ".to_string(),
                "3. b".to_string(),
                "4. c".to_string(),
            ]
        );
        // Cursor stays on the newly inserted item, after the "2. " prefix
        assert_eq!(editor_cursor(&app), (1, 3));
    }

    #[test]
    fn enter_at_last_numbered_item_does_not_renumber() {
        let mut app = App::new(None).unwrap();
        app.start_description_edit("1. a\n2. b");
        let ta = app.description_editor.as_mut().unwrap();
        // Move to end of "2. b"
        ta.move_cursor(ratatui_textarea::CursorMove::Bottom);
        ta.move_cursor(ratatui_textarea::CursorMove::End);

        handle(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())).unwrap();

        assert_eq!(
            editor_lines(&app),
            vec!["1. a".to_string(), "2. b".to_string(), "3. ".to_string()]
        );
    }

    #[test]
    fn enter_in_parent_numbered_list_skips_nested_children() {
        let mut app =
            App::new(None).unwrap();
        app.start_description_edit("1. parent\n   1. child\n   2. child\n2. parent2");
        // Cursor at end of "1. parent" (row 0)
        let ta = app.description_editor.as_mut().unwrap();
        ta.move_cursor(ratatui_textarea::CursorMove::Top);
        ta.move_cursor(ratatui_textarea::CursorMove::End);

        handle(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())).unwrap();

        // Parent "2. parent2" renumbered to 3, children untouched
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
        // Cursor at end of "   1. child" (row 1)
        let ta = app.description_editor.as_mut().unwrap();
        ta.move_cursor(ratatui_textarea::CursorMove::Top);
        ta.move_cursor(ratatui_textarea::CursorMove::Down);
        ta.move_cursor(ratatui_textarea::CursorMove::End);

        handle(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())).unwrap();

        // Sibling child renumbered, parent line "2. parent2" left alone
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
        // Existing numbers are [1, 5, 7]. Enter at end of "1. a" must
        // renumber every item in the run sequentially, not just bump by 1.
        let mut app = App::new(None).unwrap();
        app.start_description_edit("1. a\n5. b\n7. c");
        let ta = app.description_editor.as_mut().unwrap();
        ta.move_cursor(ratatui_textarea::CursorMove::Top);
        ta.move_cursor(ratatui_textarea::CursorMove::End);

        handle(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())).unwrap();

        assert_eq!(
            editor_lines(&app),
            vec![
                "1. a".to_string(),
                "2. ".to_string(),
                "3. b".to_string(),
                "4. c".to_string(),
            ]
        );
        // Cursor parked at end of new item's prefix
        assert_eq!(editor_cursor(&app), (1, 3));
    }

    #[test]
    fn enter_preserves_lists_starting_at_nonzero_number() {
        // List starts at 3 — first item's number is taken as the run's
        // start; subsequent items renumber sequentially from there.
        let mut app = App::new(None).unwrap();
        app.start_description_edit("3. a\n4. b\n4. c");
        let ta = app.description_editor.as_mut().unwrap();
        ta.move_cursor(ratatui_textarea::CursorMove::Top);
        ta.move_cursor(ratatui_textarea::CursorMove::End);

        handle(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())).unwrap();

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
        // Original numbers are all "1." (legal markdown). Inserting in
        // the middle must canonicalize the whole run.
        let mut app = App::new(None).unwrap();
        app.start_description_edit("1. a\n1. b\n1. c\n1. d");
        let ta = app.description_editor.as_mut().unwrap();
        // Cursor at end of "1. b" (row 1)
        ta.move_cursor(ratatui_textarea::CursorMove::Top);
        ta.move_cursor(ratatui_textarea::CursorMove::Down);
        ta.move_cursor(ratatui_textarea::CursorMove::End);

        handle(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())).unwrap();

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
        // The paragraph above the list is not a list item, so the run's
        // upward walk must stop there. Without that guard, the renumber
        // would never reach the inserted item and leave it as "1. ".
        let mut app = App::new(None).unwrap();
        app.start_description_edit("intro text\n1. foo\n2. bar");
        let ta = app.description_editor.as_mut().unwrap();
        // Cursor at end of "1. foo" (row 1)
        ta.move_cursor(ratatui_textarea::CursorMove::Top);
        ta.move_cursor(ratatui_textarea::CursorMove::Down);
        ta.move_cursor(ratatui_textarea::CursorMove::End);

        handle(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())).unwrap();

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
        // Two separate numbered lists separated by a paragraph. Editing
        // the second list must not touch the first.
        let mut app = App::new(None).unwrap();
        app.start_description_edit("1. orphan\nunrelated\n1. foo\n2. bar");
        let ta = app.description_editor.as_mut().unwrap();
        // Cursor at end of "1. foo" (row 2)
        ta.move_cursor(ratatui_textarea::CursorMove::Top);
        ta.move_cursor(ratatui_textarea::CursorMove::Down);
        ta.move_cursor(ratatui_textarea::CursorMove::Down);
        ta.move_cursor(ratatui_textarea::CursorMove::End);

        handle(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())).unwrap();

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

        handle(&mut app, KeyEvent::new(KeyCode::Tab, KeyModifiers::empty())).unwrap();

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

        handle(&mut app, KeyEvent::new(KeyCode::Tab, KeyModifiers::empty())).unwrap();

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

        handle(&mut app, KeyEvent::new(KeyCode::Tab, KeyModifiers::empty())).unwrap();

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

        handle(&mut app, KeyEvent::new(KeyCode::BackTab, KeyModifiers::empty())).unwrap();

        assert_eq!(
            editor_lines(&app),
            vec!["1. parent".to_string(), "2. child".to_string()]
        );
    }

    #[test]
    fn shift_tab_on_top_level_list_item_is_noop() {
        // Nothing to unnest at indent 0.
        let mut app = App::new(None).unwrap();
        app.start_description_edit("1. a\n2. b");
        let ta = app.description_editor.as_mut().unwrap();
        ta.move_cursor(ratatui_textarea::CursorMove::Bottom);
        ta.move_cursor(ratatui_textarea::CursorMove::End);

        handle(&mut app, KeyEvent::new(KeyCode::BackTab, KeyModifiers::empty())).unwrap();

        assert_eq!(
            editor_lines(&app),
            vec!["1. a".to_string(), "2. b".to_string()]
        );
    }

    #[test]
    fn deleting_blank_line_between_list_items_renumbers() {
        // Two list runs separated by a blank line. Removing the blank line
        // merges them into one run, which must be renumbered.
        let mut app = App::new(None).unwrap();
        app.start_description_edit("1. a\n\n5. b\n7. c");
        let ta = app.description_editor.as_mut().unwrap();
        // Cursor at start of the blank line (row 1)
        ta.move_cursor(ratatui_textarea::CursorMove::Top);
        ta.move_cursor(ratatui_textarea::CursorMove::Down);
        ta.move_cursor(ratatui_textarea::CursorMove::Head);

        handle(&mut app, KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty())).unwrap();

        assert_eq!(
            editor_lines(&app),
            vec!["1. a".to_string(), "2. b".to_string(), "3. c".to_string()]
        );
    }

    #[test]
    fn deleting_numbered_item_via_line_merge_renumbers() {
        // Cursor at start of "2. b"; Backspace joins it with the previous
        // line's tail. After the merge the remaining numbered items have
        // shifted positions and must be renumbered.
        let mut app = App::new(None).unwrap();
        app.start_description_edit("1. a\n2. b\n3. c");
        let ta = app.description_editor.as_mut().unwrap();
        // Cursor at start of "2. b" (row 1)
        ta.move_cursor(ratatui_textarea::CursorMove::Top);
        ta.move_cursor(ratatui_textarea::CursorMove::Down);
        ta.move_cursor(ratatui_textarea::CursorMove::Head);

        handle(&mut app, KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty())).unwrap();

        // Backspace at start of "2. b" merges it into the previous line:
        // line 0 becomes "1. a2. b" — still a "1." numbered prefix, so the
        // run continues. The trailing "3. c" gets renumbered to "2. c"
        // because the list run is now two items long.
        assert_eq!(
            editor_lines(&app),
            vec!["1. a2. b".to_string(), "2. c".to_string()]
        );
    }

    #[test]
    fn typing_does_not_trigger_renumber() {
        // Regression guard: renumbering must only happen on numbered-list
        // continuation Enter, not on every keystroke.
        let mut app = App::new(None).unwrap();
        app.start_description_edit("1. a\n5. b");
        // Cursor at end of "1. a"
        let ta = app.description_editor.as_mut().unwrap();
        ta.move_cursor(ratatui_textarea::CursorMove::Top);
        ta.move_cursor(ratatui_textarea::CursorMove::End);

        // Type a character — must not rewrite "5. b" to "2. b"
        handle(&mut app, KeyEvent::new(KeyCode::Char('x'), KeyModifiers::empty())).unwrap();

        assert_eq!(
            editor_lines(&app),
            vec!["1. ax".to_string(), "5. b".to_string()]
        );
    }
}
