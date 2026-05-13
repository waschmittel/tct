use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{App, AppMode, DialogKind, InsertTarget};
use crate::storage::{board_store, card_store, list_store};

pub fn handle(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    let grabbed = app.board.as_ref().map(|b| b.is_grabbed()).unwrap_or(false);

    match key.code {
        KeyCode::Char('q') if !grabbed => app.should_quit = true,
        KeyCode::Char('?') if !grabbed => app.mode = AppMode::Help,
        KeyCode::Char('b') if !grabbed => {
            app.board = None;
            app.reload_boards()?;
            app.mode = AppMode::BoardSelector;
        }

        // Navigation — when grabbed, these move the card instead of just the cursor
        KeyCode::Char('h') | KeyCode::Left => {
            if grabbed {
                move_card_left(app)?;
            } else if let Some(board) = &mut app.board {
                if board.selected_list > 0 {
                    board.selected_list -= 1;
                }
            }
        }
        KeyCode::Char('l') | KeyCode::Right => {
            if grabbed {
                move_card_right(app)?;
            } else if let Some(board) = &mut app.board {
                if board.selected_list < board.lists.len().saturating_sub(1) {
                    board.selected_list += 1;
                }
            }
        }
        KeyCode::Char('j') | KeyCode::Down => {
            if grabbed {
                move_card_down(app)?;
            } else if let Some(board) = &mut app.board {
                let li = board.selected_list;
                let max = board.visible_card_count(li).saturating_sub(1);
                if board.selected_card.get(li).copied().unwrap_or(0) < max {
                    board.selected_card[li] += 1;
                }
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if grabbed {
                move_card_up(app)?;
            } else if let Some(board) = &mut app.board {
                let li = board.selected_list;
                if board.selected_card.get(li).copied().unwrap_or(0) > 0 {
                    board.selected_card[li] -= 1;
                }
            }
        }

        // Grab/release toggle
        KeyCode::Char('m') => {
            if let Some(board) = &mut app.board {
                if board.is_grabbed() {
                    board.grabbed_card = None;
                    app.set_status("Card released".into());
                } else if let Some(card_id) = board.current_card_id().cloned() {
                    board.grabbed_card = Some(card_id);
                    app.set_status("Card grabbed — move with h/j/k/l, m or Esc to release".into());
                }
            }
        }
        KeyCode::Esc if grabbed => {
            if let Some(board) = &mut app.board {
                board.grabbed_card = None;
                app.set_status("Card released".into());
            }
        }

        KeyCode::Char('g') if !grabbed => {
            if let Some(board) = &mut app.board {
                let li = board.selected_list;
                if li < board.selected_card.len() {
                    board.selected_card[li] = 0;
                }
            }
        }
        KeyCode::Char('G') if !grabbed => {
            if let Some(board) = &mut app.board {
                let li = board.selected_list;
                let max = board.visible_card_count(li).saturating_sub(1);
                if li < board.selected_card.len() {
                    board.selected_card[li] = max;
                }
            }
        }
        KeyCode::Enter if !grabbed => {
            if let Some(board) = &app.board {
                if board.current_card_id().is_some() {
                    app.mode = AppMode::CardDetail;
                }
            }
        }
        KeyCode::Char('n') if !grabbed => {
            if app.board.is_some() {
                app.start_insert(InsertTarget::NewCardTitle);
            }
        }
        KeyCode::Char('N') if !grabbed => {
            if app.board.is_some() {
                app.start_insert(InsertTarget::NewListName);
            }
        }
        KeyCode::Char('e') if !grabbed => {
            if let Some(board) = &app.board {
                if let Some(card) = board.current_card() {
                    let title = card.title.clone();
                    app.start_insert_with(InsertTarget::EditCardTitle, &title);
                }
            }
        }
        KeyCode::Char('r') if !grabbed => {
            if let Some(board) = &app.board {
                if let Some(list) = board.lists.get(board.selected_list) {
                    let name = list.name.clone();
                    app.start_insert_with(InsertTarget::RenameList, &name);
                }
            }
        }
        KeyCode::Char('d') if !grabbed => {
            if let Some(board) = &app.board {
                if board.current_card_id().is_some() {
                    app.mode = AppMode::Dialog(DialogKind::ConfirmDeleteCard);
                }
            }
        }
        KeyCode::Char('D') if !grabbed => {
            if let Some(board) = &app.board {
                if !board.lists.is_empty() {
                    app.mode = AppMode::Dialog(DialogKind::ConfirmDeleteList);
                }
            }
        }
        KeyCode::Char('<') if !grabbed => {
            if let Some(board) = &mut app.board {
                if board.selected_list > 0 {
                    let i = board.selected_list;
                    board.lists.swap(i, i - 1);
                    board.meta.list_order.swap(i, i - 1);
                    board.selected_card.swap(i, i - 1);
                    board.scroll_offset.swap(i, i - 1);
                    board.selected_list -= 1;
                    board_store::save_board(&board.meta)?;
                }
            }
        }
        KeyCode::Char('>') if !grabbed => {
            if let Some(board) = &mut app.board {
                if board.selected_list < board.lists.len().saturating_sub(1) {
                    let i = board.selected_list;
                    board.lists.swap(i, i + 1);
                    board.meta.list_order.swap(i, i + 1);
                    board.selected_card.swap(i, i + 1);
                    board.scroll_offset.swap(i, i + 1);
                    board.selected_list += 1;
                    board_store::save_board(&board.meta)?;
                }
            }
        }
        KeyCode::Char('a') if !grabbed => {
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
        }
        KeyCode::Char('/') if !grabbed => {
            app.search_query.clear();
            app.mode = AppMode::Command;
        }
        KeyCode::Char('f') if !grabbed => {
            app.label_picker_idx = 0;
            app.mode = AppMode::Dialog(DialogKind::LabelPicker);
        }
        KeyCode::Char('F') if !grabbed => {
            app.search_active = false;
            app.search_query.clear();
            app.label_filter = None;
            app.set_status("Filters cleared".into());
        }
        _ => {}
    }
    Ok(())
}

fn move_card_down(app: &mut App) -> anyhow::Result<()> {
    let board = match &mut app.board {
        Some(b) => b,
        None => return Ok(()),
    };
    let li = board.selected_list;
    let ci = board.selected_card.get(li).copied().unwrap_or(0);
    if let Some(list) = board.lists.get_mut(li) {
        if ci < list.card_ids.len().saturating_sub(1) {
            list.card_ids.swap(ci, ci + 1);
            board.selected_card[li] = ci + 1;
            list_store::save_list(&board.meta.id, list)?;
        }
    }
    Ok(())
}

fn move_card_up(app: &mut App) -> anyhow::Result<()> {
    let board = match &mut app.board {
        Some(b) => b,
        None => return Ok(()),
    };
    let li = board.selected_list;
    let ci = board.selected_card.get(li).copied().unwrap_or(0);
    if ci > 0 {
        if let Some(list) = board.lists.get_mut(li) {
            list.card_ids.swap(ci, ci - 1);
            board.selected_card[li] = ci - 1;
            list_store::save_list(&board.meta.id, list)?;
        }
    }
    Ok(())
}

fn move_card_left(app: &mut App) -> anyhow::Result<()> {
    let board = match &mut app.board {
        Some(b) => b,
        None => return Ok(()),
    };
    let src = board.selected_list;
    if src == 0 {
        return Ok(());
    }
    let dst = src - 1;
    let ci = board.selected_card.get(src).copied().unwrap_or(0);

    let card_id = match board.lists.get(src).and_then(|l| l.card_ids.get(ci)) {
        Some(id) => id.clone(),
        None => return Ok(()),
    };

    board.lists[src].card_ids.remove(ci);
    list_store::save_list(&board.meta.id, &board.lists[src])?;

    board.lists[dst].card_ids.push(card_id);
    list_store::save_list(&board.meta.id, &board.lists[dst])?;

    board.clamp_selection();
    board.selected_list = dst;
    board.selected_card[dst] = board.lists[dst].card_ids.len().saturating_sub(1);
    Ok(())
}

fn move_card_right(app: &mut App) -> anyhow::Result<()> {
    let board = match &mut app.board {
        Some(b) => b,
        None => return Ok(()),
    };
    let src = board.selected_list;
    if src >= board.lists.len().saturating_sub(1) {
        return Ok(());
    }
    let dst = src + 1;
    let ci = board.selected_card.get(src).copied().unwrap_or(0);

    let card_id = match board.lists.get(src).and_then(|l| l.card_ids.get(ci)) {
        Some(id) => id.clone(),
        None => return Ok(()),
    };

    board.lists[src].card_ids.remove(ci);
    list_store::save_list(&board.meta.id, &board.lists[src])?;

    board.lists[dst].card_ids.push(card_id);
    list_store::save_list(&board.meta.id, &board.lists[dst])?;

    board.clamp_selection();
    board.selected_list = dst;
    board.selected_card[dst] = board.lists[dst].card_ids.len().saturating_sub(1);
    Ok(())
}
