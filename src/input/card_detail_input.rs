use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, AppMode, InsertTarget};

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
            if let Some(board) = &mut app.board {
                if let Some(card) = board.current_card() {
                    if board.detail_item_idx < card.checklist.len().saturating_sub(1) {
                        board.detail_item_idx += 1;
                    }
                }
            }
        }
        (KeyCode::Up, false) => {
            if let Some(board) = &mut app.board {
                if board.detail_item_idx > 0 {
                    board.detail_item_idx -= 1;
                }
            }
        }

        // Reorder checklist item down
        (KeyCode::Down, true) => {
            if let Some(board) = &mut app.board {
                if let Some(card_id) = board.current_card_id().cloned() {
                    let ii = board.detail_item_idx;
                    if let Some(card) = board.cards.get_mut(&card_id) {
                        if ii + 1 < card.checklist.len() {
                            card.checklist.swap(ii, ii + 1);
                            board.detail_item_idx += 1;
                            card.touch();
                            crate::storage::card_store::save_card(&board.meta.id, card)?;
                        }
                    }
                }
            }
        }

        // Reorder checklist item up
        (KeyCode::Up, true) => {
            if let Some(board) = &mut app.board {
                if let Some(card_id) = board.current_card_id().cloned() {
                    let ii = board.detail_item_idx;
                    if let Some(card) = board.cards.get_mut(&card_id) {
                        if ii > 0 {
                            card.checklist.swap(ii, ii - 1);
                            board.detail_item_idx -= 1;
                            card.touch();
                            crate::storage::card_store::save_card(&board.meta.id, card)?;
                        }
                    }
                }
            }
        }

        // Toggle checklist item
        (KeyCode::Char(' '), _) => {
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
        (KeyCode::Char('a'), _) => {
            app.start_insert(InsertTarget::NewChecklistItem);
        }

        // Delete checklist item
        (KeyCode::Char('x'), _) => {
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
        (KeyCode::Enter, _) => {
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
        (KeyCode::Char('e'), _) => {
            if let Some(board) = &app.board {
                if let Some(card) = board.current_card() {
                    let desc = card.description.clone();
                    app.start_description_edit(&desc);
                }
            }
        }

        // Edit title
        (KeyCode::Char('t'), _) => {
            if let Some(board) = &app.board {
                if let Some(card) = board.current_card() {
                    let title = card.title.clone();
                    app.start_insert_with(InsertTarget::EditCardTitle, &title);
                }
            }
        }

        // Copy description
        (KeyCode::Char('y'), _) => {
            if let Some(board) = &app.board {
                if let Some(card) = board.current_card() {
                    let desc = card.description.clone();
                    app.copy_to_clipboard(desc);
                }
            }
        }

        // Labels
        (KeyCode::Char('l'), _) => {
            if let Some(board) = &app.board {
                if board.current_card_id().is_some() {
                    app.label_picker_idx = 0;
                    app.mode = AppMode::Dialog(crate::app::DialogKind::LabelPicker);
                }
            }
        }
        (KeyCode::Char('L'), _) => {
            app.label_picker_idx = 0;
            app.mode = AppMode::Dialog(crate::app::DialogKind::LabelManager);
        }

        // Due date
        (KeyCode::Char('u'), _) => {
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

        // Clear due date
        (KeyCode::Char('U'), _) => {
            if let Some(board) = &mut app.board {
                if let Some(card_id) = board.current_card_id().cloned() {
                    if let Some(card) = board.cards.get_mut(&card_id) {
                        card.due_date = None;
                        card.touch();
                        crate::storage::card_store::save_card(&board.meta.id, card)?;
                        app.set_status("Due date cleared".into());
                    }
                }
            }
        }

        _ => {}
    }
    Ok(())
}
