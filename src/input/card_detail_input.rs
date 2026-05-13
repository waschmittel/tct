use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{App, AppMode, CardDetailTab, InsertTarget};

pub fn handle(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Tab => {
            if let Some(board) = &mut app.board {
                board.detail_tab = board.detail_tab.next();
                board.detail_checklist_idx = 0;
                board.detail_item_idx = 0;
            }
        }
        KeyCode::Char('e') => {
            if let Some(board) = &app.board {
                match board.detail_tab {
                    CardDetailTab::Description => {
                        if let Some(card) = board.current_card() {
                            let desc = card.description.clone();
                            app.start_description_edit(&desc);
                        }
                    }
                    CardDetailTab::DueDate => {
                        if let Some(card) = board.current_card() {
                            let date_str = card
                                .due_date
                                .map(|d| d.format("%Y-%m-%d").to_string())
                                .unwrap_or_default();
                            app.start_insert_with(InsertTarget::EditDueDate, &date_str);
                        }
                    }
                    CardDetailTab::Checklists => {
                        if let Some(card) = board.current_card() {
                            let ci = board.detail_checklist_idx;
                            let ii = board.detail_item_idx;
                            if let Some(cl) = card.checklists.get(ci) {
                                if let Some(item) = cl.items.get(ii) {
                                    let text = item.text.clone();
                                    app.start_insert_with(InsertTarget::EditChecklistItem, &text);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        KeyCode::Char('t') => {
            if let Some(board) = &app.board {
                if let Some(card) = board.current_card() {
                    let title = card.title.clone();
                    app.start_insert_with(InsertTarget::EditCardTitle, &title);
                }
            }
        }
        KeyCode::Char('j') | KeyCode::Down => {
            if let Some(board) = &mut app.board {
                if board.detail_tab == CardDetailTab::Checklists {
                    if let Some(card) = board.current_card() {
                        if let Some(cl) = card.checklists.get(board.detail_checklist_idx) {
                            if board.detail_item_idx < cl.items.len().saturating_sub(1) {
                                board.detail_item_idx += 1;
                            } else if board.detail_checklist_idx
                                < card.checklists.len().saturating_sub(1)
                            {
                                board.detail_checklist_idx += 1;
                                board.detail_item_idx = 0;
                            }
                        } else if !card.checklists.is_empty() {
                            board.detail_checklist_idx = 0;
                            board.detail_item_idx = 0;
                        }
                    }
                }
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if let Some(board) = &mut app.board {
                if board.detail_tab == CardDetailTab::Checklists {
                    if board.detail_item_idx > 0 {
                        board.detail_item_idx -= 1;
                    } else if board.detail_checklist_idx > 0 {
                        board.detail_checklist_idx -= 1;
                        if let Some(card) = board.current_card() {
                            if let Some(cl) = card.checklists.get(board.detail_checklist_idx) {
                                board.detail_item_idx = cl.items.len().saturating_sub(1);
                            }
                        }
                    }
                }
            }
        }
        KeyCode::Char(' ') => {
            if let Some(board) = &mut app.board {
                if board.detail_tab == CardDetailTab::Checklists {
                    if let Some(card_id) = board.current_card_id().cloned() {
                        if let Some(card) = board.cards.get_mut(&card_id) {
                            let ci = board.detail_checklist_idx;
                            let ii = board.detail_item_idx;
                            if let Some(cl) = card.checklists.get_mut(ci) {
                                if let Some(item) = cl.items.get_mut(ii) {
                                    item.completed = !item.completed;
                                    card.touch();
                                    crate::storage::card_store::save_card(&board.meta.id, card)?;
                                }
                            }
                        }
                    }
                }
            }
        }
        KeyCode::Char('a') => {
            if let Some(board) = &app.board {
                if board.detail_tab == CardDetailTab::Checklists {
                    if let Some(card) = board.current_card() {
                        if !card.checklists.is_empty() {
                            app.start_insert(InsertTarget::NewChecklistItem);
                        }
                    }
                }
            }
        }
        KeyCode::Char('A') => {
            let should_add = app.board.as_ref().map(|b| {
                matches!(b.detail_tab, CardDetailTab::Checklists | CardDetailTab::Description)
            }).unwrap_or(false);
            if should_add {
                app.start_insert(InsertTarget::NewChecklistTitle);
            }
        }
        KeyCode::Char('l') => {
            if let Some(board) = &app.board {
                if board.detail_tab == CardDetailTab::Labels {
                    if board.current_card_id().is_some() {
                        app.label_picker_idx = 0;
                        app.mode = AppMode::Dialog(crate::app::DialogKind::LabelPicker);
                    }
                }
            }
        }
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
        KeyCode::Char('x') => {
            if let Some(board) = &mut app.board {
                if board.detail_tab == CardDetailTab::Checklists {
                    if let Some(card_id) = board.current_card_id().cloned() {
                        if let Some(card) = board.cards.get_mut(&card_id) {
                            let ci = board.detail_checklist_idx;
                            let ii = board.detail_item_idx;
                            let should_remove_item = card.checklists.get(ci)
                                .map(|cl| ii < cl.items.len())
                                .unwrap_or(false);
                            if should_remove_item {
                                card.checklists[ci].items.remove(ii);
                                if card.checklists[ci].items.is_empty() && card.checklists.len() > 1 {
                                    card.checklists.remove(ci);
                                    if board.detail_checklist_idx > 0 {
                                        board.detail_checklist_idx -= 1;
                                    }
                                }
                                let item_count = card.checklists
                                    .get(board.detail_checklist_idx)
                                    .map(|cl| cl.items.len())
                                    .unwrap_or(0);
                                if board.detail_item_idx >= item_count && item_count > 0 {
                                    board.detail_item_idx = item_count - 1;
                                } else if item_count == 0 {
                                    board.detail_item_idx = 0;
                                }
                                card.touch();
                                crate::storage::card_store::save_card(&board.meta.id, card)?;
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}
