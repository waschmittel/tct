use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui_textarea::CursorMove;

use crate::app::{App, AppMode, InsertTarget};
use crate::model::card::Card;
use crate::model::list::CardList;
use crate::storage::{board_store, card_store, list_store};

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
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.input_buffer.clear();
            app.input_cursor = 0;
        }
        KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.input_cursor = 0;
        }
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
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
        (KeyCode::Char('s'), m) if m.contains(KeyModifiers::CONTROL) => {
            confirm_description_save(app)?;
        }
        (KeyCode::Esc, _) => {
            app.description_editor = None;
            app.mode = AppMode::CardDetail;
        }
        (KeyCode::Char('b'), m) if m.contains(KeyModifiers::CONTROL) => {
            wrap_selection_or_insert(app, "**", "**");
        }
        (KeyCode::Char('i'), m) if m.contains(KeyModifiers::CONTROL) => {
            wrap_selection_or_insert(app, "*", "*");
        }
        (KeyCode::Char('k'), m) if m.contains(KeyModifiers::CONTROL) => {
            wrap_selection_or_insert(app, "`", "`");
        }
        (KeyCode::Char('l'), m) if m.contains(KeyModifiers::CONTROL) => {
            insert_at_line_start(app, "- ");
        }
        (KeyCode::Char('t'), m) if m.contains(KeyModifiers::CONTROL) => {
            insert_table_template(app);
        }
        _ => {
            if let Some(textarea) = &mut app.description_editor {
                textarea.input(key);
            }
        }
    }
    Ok(())
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
    app.mode = AppMode::CardDetail;
    Ok(())
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

fn insert_table_template(app: &mut App) {
    let Some(textarea) = &mut app.description_editor else {
        return;
    };
    textarea.insert_str("| Column 1 | Column 2 |\n| --- | --- |\n| cell | cell |");
}

fn cancel_insert(app: &mut App) {
    let target = match &app.mode {
        AppMode::Insert(t) => t.clone(),
        _ => return,
    };
    app.mode = match target {
        InsertTarget::NewBoardName => AppMode::BoardSelector,
        InsertTarget::NewCardTitle | InsertTarget::NewListName | InsertTarget::RenameList => {
            AppMode::Normal
        }
        InsertTarget::EditCardTitle => AppMode::Normal,
        InsertTarget::EditCardDescription
        | InsertTarget::NewChecklistTitle
        | InsertTarget::NewChecklistItem
        | InsertTarget::EditChecklistItem
        | InsertTarget::EditDueDate => AppMode::CardDetail,
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
        InsertTarget::EditCardTitle => {
            if let Some(board) = &mut app.board {
                if let Some(card_id) = board.current_card_id().cloned() {
                    if let Some(card) = board.cards.get_mut(&card_id) {
                        card.title = text;
                        card.touch();
                        card_store::save_card(&board.meta.id, card)?;
                    }
                }
            }
            app.mode = AppMode::Normal;
        }
        InsertTarget::EditCardDescription => {
            // Handled by handle_description_edit — should not reach here
            app.mode = AppMode::CardDetail;
        }
        InsertTarget::NewChecklistTitle => {
            if let Some(board) = &mut app.board {
                if let Some(card_id) = board.current_card_id().cloned() {
                    if let Some(card) = board.cards.get_mut(&card_id) {
                        card.checklists.push(crate::model::card::Checklist {
                            title: text,
                            items: Vec::new(),
                        });
                        card.touch();
                        card_store::save_card(&board.meta.id, card)?;
                        board.detail_checklist_idx = card.checklists.len() - 1;
                    }
                }
            }
            app.mode = AppMode::CardDetail;
        }
        InsertTarget::NewChecklistItem => {
            if let Some(board) = &mut app.board {
                if let Some(card_id) = board.current_card_id().cloned() {
                    if let Some(card) = board.cards.get_mut(&card_id) {
                        if let Some(cl) = card.checklists.get_mut(board.detail_checklist_idx) {
                            cl.items.push(crate::model::card::ChecklistItem {
                                text,
                                completed: false,
                            });
                            card.touch();
                            card_store::save_card(&board.meta.id, card)?;
                        }
                    }
                }
            }
            app.mode = AppMode::CardDetail;
        }
        InsertTarget::EditChecklistItem => {
            if let Some(board) = &mut app.board {
                if let Some(card_id) = board.current_card_id().cloned() {
                    if let Some(card) = board.cards.get_mut(&card_id) {
                        if let Some(cl) = card.checklists.get_mut(board.detail_checklist_idx) {
                            if let Some(item) = cl.items.get_mut(board.detail_item_idx) {
                                item.text = text;
                                card.touch();
                                card_store::save_card(&board.meta.id, card)?;
                            }
                        }
                    }
                }
            }
            app.mode = AppMode::CardDetail;
        }
        InsertTarget::EditDueDate => {
            let mut status_msg = None;
            if let Some(board) = &mut app.board {
                if let Some(card_id) = board.current_card_id().cloned() {
                    if let Some(card) = board.cards.get_mut(&card_id) {
                        if text.is_empty() || text == "none" {
                            card.due_date = None;
                            status_msg = Some("Cleared due date".to_string());
                        } else if let Ok(date) =
                            chrono::NaiveDate::parse_from_str(&text, "%Y-%m-%d")
                        {
                            card.due_date = Some(date);
                            status_msg = Some(format!("Due date set to {date}"));
                        } else {
                            status_msg =
                                Some("Invalid date format. Use YYYY-MM-DD".to_string());
                        }
                        card.touch();
                        card_store::save_card(&board.meta.id, card)?;
                    }
                }
            }
            if let Some(msg) = status_msg {
                app.set_status(msg);
            }
            app.mode = AppMode::CardDetail;
        }
    }
    Ok(())
}
