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

        // Description scrolling
        (KeyCode::PageDown, _) => {
            scroll_description(app, 5, true);
        }
        (KeyCode::PageUp, _) => {
            scroll_description(app, 5, false);
        }

        // Reorder checklist item down
        (KeyCode::Down, true) => {
            if let Some(board) = &mut app.board {
                if let Some(card_id) = board.current_card_id().cloned() {
                    let ii = board.detail_item_idx;
                    if let Some(card) = board.cards.get_mut(&card_id) {
                        if ii + 1 < card.checklist.len() {
                            let name = card.checklist[ii].text.clone();
                            card.checklist.swap(ii, ii + 1);
                            board.detail_item_idx += 1;
                            card.log(format!("Reordered checklist item '{name}'"));
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
                            let name = card.checklist[ii].text.clone();
                            card.checklist.swap(ii, ii - 1);
                            board.detail_item_idx -= 1;
                            card.log(format!("Reordered checklist item '{name}'"));
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
                            let action = if item.completed {
                                format!("Completed checklist item '{}'", item.text)
                            } else {
                                format!("Uncompleted checklist item '{}'", item.text)
                            };
                            card.log(action);
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
                            let removed = card.checklist.remove(ii);
                            if board.detail_item_idx >= card.checklist.len() && !card.checklist.is_empty() {
                                board.detail_item_idx = card.checklist.len() - 1;
                            }
                            card.log(format!("Removed checklist item '{}'", removed.text));
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

        // Copy entire checklist as markdown
        (KeyCode::Char('Y'), _) => {
            if let Some(board) = &app.board {
                if let Some(card) = board.current_card() {
                    if card.checklist.is_empty() {
                        app.set_status("Checklist is empty".into());
                    } else {
                        let md = card.checklist_as_markdown();
                        app.copy_to_clipboard(md);
                    }
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
                    app.start_due_date_picker(&date_str);
                }
            }
        }

        // Clear due date
        (KeyCode::Char('U'), _) => {
            if let Some(board) = &mut app.board {
                if let Some(card_id) = board.current_card_id().cloned() {
                    if let Some(card) = board.cards.get_mut(&card_id) {
                        let was_set = card.due_date.is_some();
                        card.due_date = None;
                        if was_set {
                            card.log("Cleared due date");
                        } else {
                            card.touch();
                        }
                        crate::storage::card_store::save_card(&board.meta.id, card)?;
                        app.set_status("Due date cleared".into());
                    }
                }
            }
        }

        (KeyCode::Char('h'), _) => {
            if let Some(board) = &app.board {
                if board.current_card_id().is_some() {
                    app.previous_mode = Some(app.mode.clone());
                    app.history_scroll = 0;
                    app.mode = AppMode::Dialog(crate::app::DialogKind::CardHistory);
                }
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
