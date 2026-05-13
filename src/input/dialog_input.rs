use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{App, AppMode, DialogKind, InsertTarget};
use crate::storage::{board_store, card_store, list_store};

pub fn handle(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    let kind = match &app.mode {
        AppMode::Dialog(k) => k.clone(),
        _ => return Ok(()),
    };

    match kind {
        DialogKind::ConfirmDeleteCard => handle_confirm_delete_card(app, key),
        DialogKind::ConfirmDeleteList => handle_confirm_delete_list(app, key),
        DialogKind::ConfirmDeleteBoard => handle_confirm_delete_board(app, key),
        DialogKind::ConfirmArchiveCard => handle_confirm_archive_card(app, key),
        DialogKind::ConfirmCancelEdit => handle_confirm_cancel_edit(app, key),
        DialogKind::ArchivedCards => handle_archived_cards(app, key),
        DialogKind::LabelPicker => handle_label_picker(app, key),
        DialogKind::LabelManager => handle_label_manager(app, key),
    }
}

fn handle_confirm_delete_card(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            if let Some(board) = &mut app.board {
                if let Some(card_id) = board.current_card_id().cloned() {
                    card_store::delete_card(&board.meta.id, &card_id)?;
                    board.cards.remove(&card_id);
                    if let Some(list) = board.lists.get_mut(board.selected_list) {
                        list.card_ids.retain(|id| id != &card_id);
                        list_store::save_list(&board.meta.id, list)?;
                    }
                    board.clamp_selection();
                    app.set_status("Card deleted".into());
                }
            }
            app.mode = AppMode::Normal;
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        _ => {}
    }
    Ok(())
}

fn handle_confirm_delete_list(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            if let Some(board) = &mut app.board {
                let li = board.selected_list;
                if let Some(list) = board.lists.get(li) {
                    for card_id in &list.card_ids {
                        let _ = card_store::delete_card(&board.meta.id, card_id);
                        board.cards.remove(card_id);
                    }
                    list_store::delete_list_file(&board.meta.id, &list.id)?;
                    let list_id = list.id.clone();
                    board.meta.list_order.retain(|id| id != &list_id);
                    board.lists.remove(li);
                    board.selected_card.remove(li);
                    board.scroll_offset.remove(li);
                    board_store::save_board(&board.meta)?;
                    if board.selected_list > 0 && board.selected_list >= board.lists.len() {
                        board.selected_list = board.lists.len().saturating_sub(1);
                    }
                    app.set_status("List deleted".into());
                }
            }
            app.mode = AppMode::Normal;
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        _ => {}
    }
    Ok(())
}

fn handle_confirm_delete_board(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            if let Some(board) = app.boards.get(app.selected_board_idx) {
                let id = board.id.clone();
                board_store::delete_board(&id)?;
                app.reload_boards()?;
                app.set_status("Board deleted".into());
            }
            app.mode = AppMode::BoardSelector;
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            app.mode = AppMode::BoardSelector;
        }
        _ => {}
    }
    Ok(())
}

fn handle_confirm_archive_card(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            if let Some(board) = &mut app.board {
                if let Some(card_id) = board.current_card_id().cloned() {
                    if let Some(card) = board.cards.get_mut(&card_id) {
                        card.archived = true;
                        card.touch();
                        card_store::save_card(&board.meta.id, card)?;
                        if let Some(list) = board.lists.get_mut(board.selected_list) {
                            list.card_ids.retain(|id| id != &card_id);
                            list_store::save_list(&board.meta.id, list)?;
                        }
                        board.clamp_selection();
                        app.set_status("Card archived".into());
                    }
                }
            }
            app.mode = AppMode::Normal;
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        _ => {}
    }
    Ok(())
}

fn handle_confirm_cancel_edit(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            app.description_editor = None;
            app.description_original = None;
            app.mode = AppMode::CardDetail;
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            app.mode = AppMode::Insert(InsertTarget::EditCardDescription);
        }
        _ => {}
    }
    Ok(())
}

fn handle_archived_cards(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            if app.archived_selected < app.archived_cards.len().saturating_sub(1) {
                app.archived_selected += 1;
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if app.archived_selected > 0 {
                app.archived_selected -= 1;
            }
        }
        KeyCode::Enter => {
            if app.archived_selected < app.archived_cards.len() {
                let mut card = app.archived_cards.remove(app.archived_selected);
                card.archived = false;
                card.touch();
                let title = card.title.clone();
                if let Some(board) = &mut app.board {
                    card_store::save_card(&board.meta.id, &card)?;
                    if let Some(list) = board.lists.get_mut(board.selected_list) {
                        list.card_ids.push(card.id.clone());
                        list_store::save_list(&board.meta.id, list)?;
                    }
                    board.cards.insert(card.id.clone(), card);
                }
                app.set_status(format!("Restored '{title}'"));
                if app.archived_selected > 0 && app.archived_selected >= app.archived_cards.len() {
                    app.archived_selected = app.archived_cards.len().saturating_sub(1);
                }
                if app.archived_cards.is_empty() {
                    app.mode = AppMode::Normal;
                }
            }
        }
        KeyCode::Esc => {
            app.archived_cards.clear();
            app.mode = AppMode::Normal;
        }
        _ => {}
    }
    Ok(())
}

fn handle_label_picker(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    let label_count = app
        .board
        .as_ref()
        .map(|b| b.meta.labels.len())
        .unwrap_or(0);

    if label_count == 0 {
        if matches!(key.code, KeyCode::Esc) {
            app.mode = AppMode::CardDetail;
        }
        return Ok(());
    }

    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            if app.label_picker_idx < label_count - 1 {
                app.label_picker_idx += 1;
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if app.label_picker_idx > 0 {
                app.label_picker_idx -= 1;
            }
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            if let Some(board) = &mut app.board {
                let label_id = board
                    .meta
                    .labels
                    .get(app.label_picker_idx)
                    .map(|l| l.id.clone());
                if let Some(lid) = label_id {
                    if let Some(card_id) = board.current_card_id().cloned() {
                        if let Some(card) = board.cards.get_mut(&card_id) {
                            if let Some(pos) = card.label_ids.iter().position(|id| *id == lid) {
                                card.label_ids.remove(pos);
                            } else {
                                card.label_ids.push(lid);
                            }
                            card.touch();
                            card_store::save_card(&board.meta.id, card)?;
                        }
                    }
                }
            }
        }
        KeyCode::Esc => {
            app.mode = AppMode::CardDetail;
        }
        _ => {}
    }
    Ok(())
}

fn handle_label_manager(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    let label_count = app
        .board
        .as_ref()
        .map(|b| b.meta.labels.len())
        .unwrap_or(0);

    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            if label_count > 0 && app.label_picker_idx < label_count - 1 {
                app.label_picker_idx += 1;
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if app.label_picker_idx > 0 {
                app.label_picker_idx -= 1;
            }
        }
        KeyCode::Char('n') => {
            app.start_insert(InsertTarget::NewLabelName);
        }
        KeyCode::Char('e') => {
            if label_count > 0 {
                if let Some(board) = &app.board {
                    if let Some(label) = board.meta.labels.get(app.label_picker_idx) {
                        let name = label.name.clone();
                        app.start_insert_with(InsertTarget::EditLabelName, &name);
                    }
                }
            }
        }
        KeyCode::Char('c') => {
            if label_count > 0 {
                if let Some(board) = &mut app.board {
                    if let Some(label) = board.meta.labels.get_mut(app.label_picker_idx) {
                        label.color = label.color.next();
                        board_store::save_board(&board.meta)?;
                    }
                }
            }
        }
        KeyCode::Char('x') => {
            if label_count > 0 {
                if let Some(board) = &mut app.board {
                    let removed_id = board.meta.labels[app.label_picker_idx].id.clone();
                    board.meta.labels.remove(app.label_picker_idx);
                    // Remove from all cards
                    for card in board.cards.values_mut() {
                        card.label_ids.retain(|id| *id != removed_id);
                    }
                    board_store::save_board(&board.meta)?;
                    if app.label_picker_idx >= board.meta.labels.len()
                        && !board.meta.labels.is_empty()
                    {
                        app.label_picker_idx = board.meta.labels.len() - 1;
                    }
                }
            }
        }
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        _ => {}
    }
    Ok(())
}
