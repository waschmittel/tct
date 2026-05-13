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
                app.input_cursor -= 1;
                app.input_buffer.remove(app.input_cursor);
            }
        }
        KeyCode::Delete => {
            if app.input_cursor < app.input_buffer.len() {
                app.input_buffer.remove(app.input_cursor);
            }
        }
        KeyCode::Left => {
            if app.input_cursor > 0 {
                app.input_cursor -= 1;
            }
        }
        KeyCode::Right => {
            if app.input_cursor < app.input_buffer.len() {
                app.input_cursor += 1;
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
            app.input_cursor += 1;
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
                app.mode = AppMode::CardDetail;
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
        _ => {
            if let Some(textarea) = &mut app.description_editor {
                textarea.input(key);
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
    app.mode = AppMode::CardDetail;
    Ok(())
}

fn handle_enter_in_list(app: &mut App) {
    let Some(textarea) = &mut app.description_editor else {
        return;
    };
    let ratatui_textarea::DataCursor(row, _col) = textarea.cursor();
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
        if let Ok(num) = num_part.parse::<u64>() {
            let indent = current_line.len() - trimmed.len();
            let indent_str = " ".repeat(indent);
            textarea.move_cursor(CursorMove::End);
            textarea.insert_newline();
            textarea.insert_str(&format!("{indent_str}{}. ", num + 1));
            return;
        }
    }

    textarea.insert_newline();
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
    let target = match &app.mode {
        AppMode::Insert(t) => t.clone(),
        _ => return,
    };
    app.mode = match target {
        InsertTarget::NewBoardName => AppMode::BoardSelector,
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
            board_store::create_board(text.clone())?;
            app.reload_boards()?;
            app.set_status(format!("Created board '{text}'"));
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
            let return_mode = if target == InsertTarget::EditCardTitleInline {
                AppMode::Normal
            } else {
                AppMode::CardDetail
            };
            if let Some(board) = &mut app.board {
                if let Some(card_id) = board.current_card_id().cloned() {
                    if let Some(card) = board.cards.get_mut(&card_id) {
                        card.title = text;
                        card.touch();
                        card_store::save_card(&board.meta.id, card)?;
                    }
                }
            }
            app.mode = return_mode;
        }
        InsertTarget::EditCardDescription => {
            app.mode = AppMode::CardDetail;
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
            app.mode = AppMode::CardDetail;
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
            app.mode = AppMode::CardDetail;
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
                app.mode = AppMode::CardDetail;
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
                app.mode = AppMode::CardDetail;
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
