//! Description editor input handling (Ctrl-S save, list autocontinue, etc).
//!
//! Owns the `Insert(EditCardDescription)` mode. Most of the actual list
//! editing logic lives in [`super::list_editing`]; this module handles
//! keybinding dispatch and editor-wide concerns (scroll, cursor, save).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui_textarea::CursorMove;

use super::list_editing::{
    handle_enter_in_list, handle_shift_tab_unnest, handle_tab_nest, renumber_all,
};
use super::has_ctrl_or_cmd;
use crate::app::{App, AppMode, DialogKind};
use crate::storage::card_store;

pub(super) fn handle(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
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
            if !handle_tab_nest(app)
                && let Some(textarea) = &mut app.description_editor
            {
                textarea.input(key);
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
    let (current_vrow, visual_col) =
        markdown::source_to_visual(&visual_map, cursor_row, cursor_col);

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
    let target_col =
        target_src_offset + (visual_col.saturating_sub(target_vindent)).min(actual_target_vlen);

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
    let original = app.description_original.take();
    if let Some(board) = &mut app.board
        && let Some(card_id) = board.current_card_id().cloned()
        && let Some(card) = board.cards.get_mut(&card_id)
    {
        let changed = original.as_deref() != Some(text.as_str());
        card.description = text;
        if changed {
            card.log("Edited description");
        } else {
            card.touch();
        }
        card_store::save_card(&board.meta.id, card)?;
        app.set_status("Description saved".into());
    }
    app.mode = app.previous_mode.take().unwrap_or(AppMode::CardDetail);
    Ok(())
}

fn wrap_selection_or_insert(app: &mut App, prefix: &str, suffix: &str) {
    let Some(textarea) = &mut app.description_editor else {
        return;
    };
    if textarea.is_selecting() {
        textarea.cut();
        let selected = textarea.yank_text().to_string();
        textarea.insert_str(format!("{prefix}{selected}{suffix}"));
    } else {
        textarea.insert_str(format!("{prefix}{suffix}"));
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
