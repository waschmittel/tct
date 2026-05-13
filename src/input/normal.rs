use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{App, AppMode, DialogKind, GrabOrigin, InsertTarget};
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

        // Navigation — when grabbed, these move the card
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
            } else if app.search_active {
                if let Some(board) = &mut app.board {
                    let li = board.selected_list;
                    let current = board.selected_card.get(li).copied().unwrap_or(0);
                    if let Some(next) = next_matching_card(board, li, current, &app.search_query) {
                        board.selected_card[li] = next;
                    }
                }
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
            } else if app.search_active {
                if let Some(board) = &mut app.board {
                    let li = board.selected_list;
                    let current = board.selected_card.get(li).copied().unwrap_or(0);
                    if let Some(prev) = prev_matching_card(board, li, current, &app.search_query) {
                        board.selected_card[li] = prev;
                    }
                }
            } else if let Some(board) = &mut app.board {
                let li = board.selected_list;
                if board.selected_card.get(li).copied().unwrap_or(0) > 0 {
                    board.selected_card[li] -= 1;
                }
            }
        }

        // Grab / confirm / abort
        KeyCode::Char('M') => {
            if let Some(board) = &mut app.board {
                if board.is_grabbed() {
                    // Also confirm (like Enter)
                    board.grabbed_card = None;
                    board.grab_origin = None;
                    app.set_status("Card placed".into());
                } else if let Some(card_id) = board.current_card_id().cloned() {
                    let origin = GrabOrigin {
                        list_idx: board.selected_list,
                        card_idx: board.selected_card[board.selected_list],
                    };
                    board.grabbed_card = Some(card_id);
                    board.grab_origin = Some(origin);
                    app.set_status("Card grabbed — h/j/k/l to move, Enter to confirm, Esc to cancel".into());
                }
            }
        }
        KeyCode::Enter if grabbed => {
            if let Some(board) = &mut app.board {
                board.grabbed_card = None;
                board.grab_origin = None;
                app.set_status("Card placed".into());
            }
        }
        KeyCode::Esc if grabbed => {
            abort_card_move(app)?;
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
        KeyCode::Char('e') if !grabbed => {
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
        KeyCode::Enter if !grabbed => {
            if let Some(board) = &app.board {
                if let Some(card) = board.current_card() {
                    let title = card.title.clone();
                    app.start_insert_with(InsertTarget::EditCardTitleInline, &title);
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
            if let Some(board) = &app.board {
                if board.current_card_id().is_some() {
                    app.mode = AppMode::Dialog(DialogKind::ConfirmArchiveCard);
                }
            }
        }
        KeyCode::Char('v') if !grabbed => {
            if let Some(board) = &app.board {
                let archived = card_store::list_archived_cards(&board.meta.id);
                if archived.is_empty() {
                    app.set_status("No archived cards".into());
                } else {
                    app.archived_cards = archived;
                    app.archived_selected = 0;
                    app.mode = AppMode::Dialog(DialogKind::ArchivedCards);
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
        KeyCode::Char('L') if !grabbed => {
            app.label_picker_idx = 0;
            app.mode = AppMode::Dialog(DialogKind::LabelManager);
        }
        _ => {}
    }
    Ok(())
}

fn abort_card_move(app: &mut App) -> anyhow::Result<()> {
    let board = match &mut app.board {
        Some(b) => b,
        None => return Ok(()),
    };

    let origin = match board.grab_origin.take() {
        Some(o) => o,
        None => {
            board.grabbed_card = None;
            return Ok(());
        }
    };

    let card_id = match board.grabbed_card.take() {
        Some(id) => id,
        None => return Ok(()),
    };

    // Find where the card currently is
    let mut current_list = None;
    let mut current_idx = None;
    for (li, list) in board.lists.iter().enumerate() {
        if let Some(ci) = list.card_ids.iter().position(|id| *id == card_id) {
            current_list = Some(li);
            current_idx = Some(ci);
            break;
        }
    }

    if let (Some(cur_li), Some(cur_ci)) = (current_list, current_idx) {
        // Remove from current position
        board.lists[cur_li].card_ids.remove(cur_ci);
        list_store::save_list(&board.meta.id, &board.lists[cur_li])?;

        // Insert at original position
        let dest_li = origin.list_idx.min(board.lists.len().saturating_sub(1));
        let dest_ci = origin.card_idx.min(board.lists[dest_li].card_ids.len());
        board.lists[dest_li].card_ids.insert(dest_ci, card_id);
        if cur_li != dest_li {
            list_store::save_list(&board.meta.id, &board.lists[dest_li])?;
        }

        board.selected_list = dest_li;
        board.clamp_selection();
        board.selected_card[dest_li] = dest_ci.min(board.visible_card_count(dest_li).saturating_sub(1));
    }

    app.set_status("Move cancelled".into());
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

fn visible_card_ids(board: &crate::app::LoadedBoard, list_idx: usize) -> Vec<usize> {
    let list = match board.lists.get(list_idx) {
        Some(l) => l,
        None => return vec![],
    };
    list.card_ids
        .iter()
        .enumerate()
        .filter(|(_, id)| {
            board
                .cards
                .get(*id)
                .map(|c| !c.archived)
                .unwrap_or(false)
        })
        .map(|(i, _)| i)
        .collect()
}

fn next_matching_card(
    board: &crate::app::LoadedBoard,
    list_idx: usize,
    current: usize,
    query: &str,
) -> Option<usize> {
    let indices = visible_card_ids(board, list_idx);
    let list = board.lists.get(list_idx)?;
    for &i in &indices {
        if i > current {
            let card_id = &list.card_ids[i];
            if let Some(card) = board.cards.get(card_id) {
                if card.matches_search(query, &board.meta.labels) {
                    return Some(i);
                }
            }
        }
    }
    None
}

fn prev_matching_card(
    board: &crate::app::LoadedBoard,
    list_idx: usize,
    current: usize,
    query: &str,
) -> Option<usize> {
    let indices = visible_card_ids(board, list_idx);
    let list = board.lists.get(list_idx)?;
    for &i in indices.iter().rev() {
        if i < current {
            let card_id = &list.card_ids[i];
            if let Some(card) = board.cards.get(card_id) {
                if card.matches_search(query, &board.meta.labels) {
                    return Some(i);
                }
            }
        }
    }
    None
}
