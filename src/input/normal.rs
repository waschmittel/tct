use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, AppMode, DialogKind, InsertTarget};
use crate::storage::{board_store, card_store, list_store};

pub fn handle(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);

    match (key.code, shift) {
        (KeyCode::Char('q'), _) => app.should_quit = true,
        (KeyCode::Char('?'), _) => {
            app.previous_mode = Some(app.mode.clone());
            app.mode = AppMode::Help;
        }
        (KeyCode::Char('b'), _) => {
            app.board = None;
            app.reload_boards()?;
            app.mode = AppMode::BoardSelector;
        }

        // Navigation — arrow keys only (no h/j/k/l)
        (KeyCode::Left, false) => {
            if let Some(board) = &mut app.board {
                if board.selected_list > 0 {
                    board.selected_list -= 1;
                }
            }
        }
        (KeyCode::Right, false) => {
            if let Some(board) = &mut app.board {
                if board.selected_list < board.lists.len().saturating_sub(1) {
                    board.selected_list += 1;
                }
            }
        }
        (KeyCode::Down, false) => {
            if app.search_active {
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
        (KeyCode::Up, false) => {
            if app.search_active {
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

        // Shift+Arrow: move card
        (KeyCode::Left, true) => move_card_left(app)?,
        (KeyCode::Right, true) => move_card_right(app)?,
        (KeyCode::Up, true) => move_card_up(app)?,
        (KeyCode::Down, true) => move_card_down(app)?,

        (KeyCode::Char('g'), _) => {
            if let Some(board) = &mut app.board {
                let li = board.selected_list;
                if li < board.selected_card.len() {
                    board.selected_card[li] = 0;
                }
            }
        }
        (KeyCode::Char('G'), _) => {
            if let Some(board) = &mut app.board {
                let li = board.selected_list;
                let max = board.visible_card_count(li).saturating_sub(1);
                if li < board.selected_card.len() {
                    board.selected_card[li] = max;
                }
            }
        }

        // Enter: open card detail (swapped — was 'e')
        (KeyCode::Enter, _) => {
            if let Some(board) = &app.board {
                if board.current_card_id().is_some() {
                    app.mode = AppMode::CardDetail;
                }
            }
        }

        (KeyCode::Char('y'), _) => {
            if let Some(board) = &app.board {
                if let Some(card) = board.current_card() {
                    let title = card.title.clone();
                    app.copy_to_clipboard(title);
                }
            }
        }

        // e: quick-edit card title inline (swapped — was Enter)
        (KeyCode::Char('e'), _) => {
            if let Some(board) = &app.board {
                if let Some(card) = board.current_card() {
                    let title = card.title.clone();
                    app.start_insert_with(InsertTarget::EditCardTitleInline, &title);
                }
            }
        }

        (KeyCode::Char('n'), _) => {
            if app.board.is_some() {
                app.start_insert(InsertTarget::NewCardTitle);
            }
        }
        (KeyCode::Char('N'), _) => {
            if app.board.is_some() {
                app.start_insert(InsertTarget::NewListName);
            }
        }
        (KeyCode::Char('r'), _) => {
            if let Some(board) = &app.board {
                if let Some(list) = board.lists.get(board.selected_list) {
                    let name = list.name.clone();
                    app.start_insert_with(InsertTarget::RenameList, &name);
                }
            }
        }
        (KeyCode::Char('d'), _) => {
            if let Some(board) = &app.board {
                if board.current_card_id().is_some() {
                    app.mode = AppMode::Dialog(DialogKind::ConfirmDeleteCard);
                }
            }
        }
        (KeyCode::Char('D'), _) => {
            if let Some(board) = &app.board {
                if !board.lists.is_empty() {
                    app.mode = AppMode::Dialog(DialogKind::ConfirmDeleteList);
                }
            }
        }
        (KeyCode::Char('<'), _) => {
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
        (KeyCode::Char('>'), _) => {
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
        (KeyCode::Char('a'), _) => {
            if let Some(board) = &app.board {
                if board.current_card_id().is_some() {
                    app.mode = AppMode::Dialog(DialogKind::ConfirmArchiveCard);
                }
            }
        }
        (KeyCode::Char('v'), _) => {
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
        (KeyCode::Char('/'), _) => {
            app.search_query.clear();
            app.mode = AppMode::Command;
        }
        (KeyCode::Char('f'), _) => {
            app.label_picker_idx = 0;
            app.mode = AppMode::Dialog(DialogKind::LabelPicker);
        }
        (KeyCode::Char('F'), _) => {
            app.search_active = false;
            app.search_query.clear();
            app.label_filter = None;
            app.set_status("Filters cleared".into());
        }
        (KeyCode::Char('l'), _) => {
            if let Some(board) = &app.board {
                if board.current_card_id().is_some() {
                    app.previous_mode = Some(app.mode.clone());
                    app.label_picker_idx = 0;
                    app.mode = AppMode::Dialog(DialogKind::LabelPicker);
                }
            }
        }
        (KeyCode::Char('L'), _) => {
            app.label_picker_idx = 0;
            app.mode = AppMode::Dialog(DialogKind::LabelManager);
        }
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
        (KeyCode::Char('U'), _) => {
            if let Some(board) = &mut app.board {
                if let Some(card_id) = board.current_card_id().cloned() {
                    if let Some(card) = board.cards.get_mut(&card_id) {
                        card.due_date = None;
                        card.touch();
                        card_store::save_card(&board.meta.id, card)?;
                        app.set_status("Due date cleared".into());
                    }
                }
            }
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

    let insert_at = ci.min(board.lists[dst].card_ids.len());
    board.lists[dst].card_ids.insert(insert_at, card_id);
    list_store::save_list(&board.meta.id, &board.lists[dst])?;

    board.clamp_selection();
    board.selected_list = dst;
    board.selected_card[dst] = insert_at;
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

    let insert_at = ci.min(board.lists[dst].card_ids.len());
    board.lists[dst].card_ids.insert(insert_at, card_id);
    list_store::save_list(&board.meta.id, &board.lists[dst])?;

    board.clamp_selection();
    board.selected_list = dst;
    board.selected_card[dst] = insert_at;
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
