use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{App, AppMode, InsertTarget};

pub fn handle(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    match key.code {
        KeyCode::Esc => {
            if let Some(board) = &mut app.board {
                board.detail_item_idx = 0;
                board.detail_scroll = 0;
            }
            app.mode = AppMode::Normal;
        }

        // Checklist item navigation
        KeyCode::Char('j') | KeyCode::Down => {
            if let Some(board) = &mut app.board {
                if let Some(card) = board.current_card() {
                    if board.detail_item_idx < card.checklist.len().saturating_sub(1) {
                        board.detail_item_idx += 1;
                    }
                }
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if let Some(board) = &mut app.board {
                if board.detail_item_idx > 0 {
                    board.detail_item_idx -= 1;
                }
            }
        }

        // Toggle checklist item
        KeyCode::Char(' ') => {
            if let Some(board) = &mut app.board {
                if let Some(card_id) = board.current_card_id().cloned() {
                    let ii = board.detail_item_idx;
                    if let Some(card) = board.cards.get_mut(&card_id) {
                        if let Some(item) = card.checklist.get_mut(ii) {
                            item.completed = !item.completed;
                            card.touch();
                            crate::storage::card_store::save_card(&board.meta.id, card)?;
                        }
                    }
                }
            }
        }

        // Add checklist item
        KeyCode::Char('a') => {
            app.start_insert(InsertTarget::NewChecklistItem);
        }

        // Delete checklist item
        KeyCode::Char('x') => {
            if let Some(board) = &mut app.board {
                if let Some(card_id) = board.current_card_id().cloned() {
                    let ii = board.detail_item_idx;
                    if let Some(card) = board.cards.get_mut(&card_id) {
                        if ii < card.checklist.len() {
                            card.checklist.remove(ii);
                            if board.detail_item_idx >= card.checklist.len() && !card.checklist.is_empty() {
                                board.detail_item_idx = card.checklist.len() - 1;
                            }
                            card.touch();
                            crate::storage::card_store::save_card(&board.meta.id, card)?;
                        }
                    }
                }
            }
        }

        // Edit checklist item text
        KeyCode::Enter => {
            if let Some(board) = &app.board {
                if let Some(card) = board.current_card() {
                    let ii = board.detail_item_idx;
                    if let Some(item) = card.checklist.get(ii) {
                        let text = item.text.clone();
                        app.start_insert_with(InsertTarget::EditChecklistItem, &text);
                    }
                }
            }
        }

        // Edit description
        KeyCode::Char('e') => {
            if let Some(board) = &app.board {
                if let Some(card) = board.current_card() {
                    let desc = card.description.clone();
                    app.start_description_edit(&desc);
                }
            }
        }

        // Edit title
        KeyCode::Char('t') => {
            if let Some(board) = &app.board {
                if let Some(card) = board.current_card() {
                    let title = card.title.clone();
                    app.start_insert_with(InsertTarget::EditCardTitle, &title);
                }
            }
        }

        // Labels
        KeyCode::Char('l') => {
            if let Some(board) = &app.board {
                if board.current_card_id().is_some() {
                    app.label_picker_idx = 0;
                    app.mode = AppMode::Dialog(crate::app::DialogKind::LabelPicker);
                }
            }
        }
        KeyCode::Char('L') => {
            app.label_picker_idx = 0;
            app.mode = AppMode::Dialog(crate::app::DialogKind::LabelManager);
        }

        // Due date
        KeyCode::Char('u') => {
            if let Some(board) = &app.board {
                if let Some(card) = board.current_card() {
                    let date_str = card
                        .due_date
                        .map(|d| d.format("%Y-%m-%d").to_string())
                        .unwrap_or_default();
                    app.start_insert_with(InsertTarget::EditDueDate, &date_str);
                }
            }
        }

        _ => {}
    }
    Ok(())
}
