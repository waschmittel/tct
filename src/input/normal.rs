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
            if let Some(board) = &mut app.board {
                if board.current_card_id().is_some() {
                    board.detail_item_idx = 0;
                    board.detail_scroll = 0;
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

        // e: edit description (consistent with card detail view)
        (KeyCode::Char('e'), _) => {
            if let Some(board) = &app.board {
                if let Some(card) = board.current_card() {
                    let desc = card.description.clone();
                    app.start_description_edit(&desc);
                }
            }
        }

        // t: edit card title inline (consistent with card detail view)
        (KeyCode::Char('t'), _) => {
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
        (KeyCode::Char('D'), _) => {
            if let Some(board) = &app.board {
                if !board.lists.is_empty() {
                    app.mode = AppMode::Dialog(DialogKind::ConfirmArchiveList);
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
        (KeyCode::Char('V'), _) => {
            if let Some(board) = &app.board {
                let archived = list_store::list_archived_lists(&board.meta.id);
                if archived.is_empty() {
                    app.set_status("No archived lists".into());
                } else {
                    app.archived_lists = archived;
                    app.archived_selected = 0;
                    app.mode = AppMode::Dialog(DialogKind::ArchivedLists);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, LoadedBoard};
    use crate::model::board::BoardMeta;
    use crate::model::card::Card;
    use crate::model::list::CardList;
    use crate::storage::{board_store, card_store, list_store};
    use crate::test_support::with_temp_dir;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn shift_key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    /// Build an App with a single list of cards with the given titles.
    fn single_list_fixture(titles: &[&str]) -> (App, BoardMeta) {
        let mut meta = board_store::create_board("Board".into()).unwrap();
        let mut list = CardList::new("L".into());
        for t in titles {
            let c = Card::new((*t).into());
            card_store::save_card(&meta.id, &c).unwrap();
            list.card_ids.push(c.id.clone());
        }
        list_store::save_list(&meta.id, &list).unwrap();
        meta.list_order = vec![list.id.clone()];
        board_store::save_board(&meta).unwrap();
        (App::new(Some(meta.id.clone())).unwrap(), meta)
    }

    /// Build a `LoadedBoard` directly (no disk) with cards in one list.
    fn loaded_board(cards: Vec<Card>) -> LoadedBoard {
        let card_ids: Vec<_> = cards.iter().map(|c| c.id.clone()).collect();
        let cards_map: std::collections::HashMap<_, _> =
            cards.into_iter().map(|c| (c.id.clone(), c)).collect();
        LoadedBoard {
            meta: BoardMeta::new("X".into()),
            lists: vec![CardList { id: "l".into(), name: "L".into(), card_ids, archived: false }],
            cards: cards_map,
            selected_list: 0,
            selected_card: vec![0],
            scroll_offset: vec![0],
            detail_item_idx: 0,
            detail_scroll: 0,
        }
    }

    fn fixed_card(id: &str, title: &str) -> Card {
        let mut c = Card::new(title.into());
        c.id = id.into();
        c
    }

    /// Build an App with a board having two lists; list 0 has 3 cards, list 1 has 1.
    fn fixture() -> (App, BoardMeta, Vec<Card>, Vec<Card>) {
        let mut meta = board_store::create_board("Board".into()).unwrap();
        let mut list_a = CardList::new("A".into());
        let mut list_b = CardList::new("B".into());

        let a_cards: Vec<Card> = (0..3).map(|i| Card::new(format!("a{i}"))).collect();
        let b_cards = vec![Card::new("b0".into())];

        for c in &a_cards {
            card_store::save_card(&meta.id, c).unwrap();
            list_a.card_ids.push(c.id.clone());
        }
        for c in &b_cards {
            card_store::save_card(&meta.id, c).unwrap();
            list_b.card_ids.push(c.id.clone());
        }
        list_store::save_list(&meta.id, &list_a).unwrap();
        list_store::save_list(&meta.id, &list_b).unwrap();
        meta.list_order = vec![list_a.id.clone(), list_b.id.clone()];
        board_store::save_board(&meta).unwrap();

        let app = App::new(Some(meta.id.clone())).unwrap();
        (app, meta, a_cards, b_cards)
    }

    #[test]
    fn down_moves_selection() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            assert_eq!(app.board.as_ref().unwrap().selected_card[0], 0);
            handle(&mut app, key(KeyCode::Down)).unwrap();
            assert_eq!(app.board.as_ref().unwrap().selected_card[0], 1);
        });
    }

    #[test]
    fn down_stops_at_last_card() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            for _ in 0..10 {
                handle(&mut app, key(KeyCode::Down)).unwrap();
            }
            // Max for list 0 (3 cards) is index 2
            assert_eq!(app.board.as_ref().unwrap().selected_card[0], 2);
        });
    }

    #[test]
    fn up_stops_at_zero() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            for _ in 0..5 {
                handle(&mut app, key(KeyCode::Up)).unwrap();
            }
            assert_eq!(app.board.as_ref().unwrap().selected_card[0], 0);
        });
    }

    #[test]
    fn right_moves_to_next_list() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            handle(&mut app, key(KeyCode::Right)).unwrap();
            assert_eq!(app.board.as_ref().unwrap().selected_list, 1);
        });
    }

    #[test]
    fn right_stops_at_last_list() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            for _ in 0..5 {
                handle(&mut app, key(KeyCode::Right)).unwrap();
            }
            assert_eq!(app.board.as_ref().unwrap().selected_list, 1);
        });
    }

    #[test]
    fn left_stops_at_zero() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            for _ in 0..5 {
                handle(&mut app, key(KeyCode::Left)).unwrap();
            }
            assert_eq!(app.board.as_ref().unwrap().selected_list, 0);
        });
    }

    #[test]
    fn g_jumps_to_top() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            app.board.as_mut().unwrap().selected_card[0] = 2;
            handle(&mut app, key(KeyCode::Char('g'))).unwrap();
            assert_eq!(app.board.as_ref().unwrap().selected_card[0], 0);
        });
    }

    #[test]
    fn shift_g_jumps_to_bottom() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            handle(&mut app, shift_key(KeyCode::Char('G'))).unwrap();
            assert_eq!(app.board.as_ref().unwrap().selected_card[0], 2);
        });
    }

    #[test]
    fn shift_down_moves_card_within_list() {
        with_temp_dir(|| {
            let (mut app, _, a_cards, _) = fixture();
            // Cards initially [a0, a1, a2]; selection at 0
            handle(&mut app, shift_key(KeyCode::Down)).unwrap();
            let board = app.board.as_ref().unwrap();
            assert_eq!(board.lists[0].card_ids[0], a_cards[1].id);
            assert_eq!(board.lists[0].card_ids[1], a_cards[0].id);
            assert_eq!(board.selected_card[0], 1);
        });
    }

    #[test]
    fn shift_up_at_top_is_noop() {
        with_temp_dir(|| {
            let (mut app, _, a_cards, _) = fixture();
            handle(&mut app, shift_key(KeyCode::Up)).unwrap();
            let board = app.board.as_ref().unwrap();
            assert_eq!(board.lists[0].card_ids[0], a_cards[0].id);
            assert_eq!(board.selected_card[0], 0);
        });
    }

    #[test]
    fn shift_down_at_bottom_is_noop() {
        with_temp_dir(|| {
            let (mut app, _, a_cards, _) = fixture();
            app.board.as_mut().unwrap().selected_card[0] = 2;
            handle(&mut app, shift_key(KeyCode::Down)).unwrap();
            let board = app.board.as_ref().unwrap();
            assert_eq!(board.lists[0].card_ids[2], a_cards[2].id);
            assert_eq!(board.selected_card[0], 2);
        });
    }

    #[test]
    fn shift_right_moves_card_to_next_list() {
        with_temp_dir(|| {
            let (mut app, _, a_cards, _) = fixture();
            handle(&mut app, shift_key(KeyCode::Right)).unwrap();
            let board = app.board.as_ref().unwrap();
            // a0 moved to list 1 at index 0
            assert_eq!(board.lists[0].card_ids.len(), 2);
            assert!(board.lists[1].card_ids.contains(&a_cards[0].id));
            assert_eq!(board.selected_list, 1);
        });
    }

    #[test]
    fn shift_left_from_first_list_is_noop() {
        with_temp_dir(|| {
            let (mut app, _, a_cards, _) = fixture();
            handle(&mut app, shift_key(KeyCode::Left)).unwrap();
            let board = app.board.as_ref().unwrap();
            assert_eq!(board.lists[0].card_ids.len(), 3);
            assert_eq!(board.lists[0].card_ids[0], a_cards[0].id);
        });
    }

    #[test]
    fn shift_right_from_last_list_is_noop() {
        with_temp_dir(|| {
            let (mut app, _, _, b_cards) = fixture();
            handle(&mut app, key(KeyCode::Right)).unwrap();
            assert_eq!(app.board.as_ref().unwrap().selected_list, 1);
            handle(&mut app, shift_key(KeyCode::Right)).unwrap();
            let board = app.board.as_ref().unwrap();
            assert_eq!(board.lists[1].card_ids.len(), 1);
            assert_eq!(board.lists[1].card_ids[0], b_cards[0].id);
        });
    }

    #[test]
    fn q_sets_should_quit() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            handle(&mut app, key(KeyCode::Char('q'))).unwrap();
            assert!(app.should_quit);
        });
    }

    #[test]
    fn question_enters_help() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            handle(&mut app, key(KeyCode::Char('?'))).unwrap();
            assert_eq!(app.mode, AppMode::Help);
        });
    }

    #[test]
    fn b_returns_to_board_selector() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            handle(&mut app, key(KeyCode::Char('b'))).unwrap();
            assert!(app.board.is_none());
            assert_eq!(app.mode, AppMode::BoardSelector);
        });
    }

    #[test]
    fn enter_opens_card_detail() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            handle(&mut app, key(KeyCode::Enter)).unwrap();
            assert_eq!(app.mode, AppMode::CardDetail);
        });
    }

    #[test]
    fn slash_enters_command_and_clears_query() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            app.search_query = "old".into();
            handle(&mut app, key(KeyCode::Char('/'))).unwrap();
            assert_eq!(app.mode, AppMode::Command);
            assert!(app.search_query.is_empty());
        });
    }

    #[test]
    fn shift_f_clears_filters() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            app.search_active = true;
            app.search_query = "x".into();
            app.label_filter = Some(crate::model::label::LabelColor::Red);
            handle(&mut app, shift_key(KeyCode::Char('F'))).unwrap();
            assert!(!app.search_active);
            assert!(app.search_query.is_empty());
            assert!(app.label_filter.is_none());
        });
    }

    #[test]
    fn shift_u_clears_due_date() {
        with_temp_dir(|| {
            let (mut app, meta, a_cards, _) = fixture();
            // Set a due date on selected card
            let cid = a_cards[0].id.clone();
            {
                let board = app.board.as_mut().unwrap();
                let card = board.cards.get_mut(&cid).unwrap();
                card.due_date = Some(chrono::NaiveDate::from_ymd_opt(2099, 1, 1).unwrap());
                card_store::save_card(&meta.id, card).unwrap();
            }
            handle(&mut app, shift_key(KeyCode::Char('U'))).unwrap();
            let board = app.board.as_ref().unwrap();
            assert!(board.cards.get(&cid).unwrap().due_date.is_none());
            // Verify persisted
            let on_disk = card_store::load_card(&meta.id, &cid).unwrap();
            assert!(on_disk.due_date.is_none());
        });
    }

    #[test]
    fn search_nav_skips_non_matching_cards() {
        with_temp_dir(|| {
            let (mut app, _) = single_list_fixture(&["alpha", "BINGO match", "gamma"]);
            app.search_active = true;
            app.search_query = "BINGO".into();
            // From index 0, next match is index 1
            handle(&mut app, key(KeyCode::Down)).unwrap();
            assert_eq!(app.board.as_ref().unwrap().selected_card[0], 1);
            // From index 1, no further match → stays at 1
            handle(&mut app, key(KeyCode::Down)).unwrap();
            assert_eq!(app.board.as_ref().unwrap().selected_card[0], 1);
        });
    }

    #[test]
    fn search_nav_up_finds_previous_match() {
        with_temp_dir(|| {
            let (mut app, _) = single_list_fixture(&["BINGO one", "nope", "BINGO three"]);
            app.search_active = true;
            app.search_query = "BINGO".into();
            app.board.as_mut().unwrap().selected_card[0] = 2;
            handle(&mut app, key(KeyCode::Up)).unwrap();
            assert_eq!(app.board.as_ref().unwrap().selected_card[0], 0);
        });
    }

    #[test]
    fn shift_left_arrow_swaps_lists() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            handle(&mut app, key(KeyCode::Right)).unwrap(); // move to list 1
            handle(&mut app, shift_key(KeyCode::Char('<'))).unwrap();
            let board = app.board.as_ref().unwrap();
            assert_eq!(board.lists[0].name, "B");
            assert_eq!(board.lists[1].name, "A");
            assert_eq!(board.selected_list, 0);
        });
    }

    #[test]
    fn shift_right_arrow_swaps_lists() {
        with_temp_dir(|| {
            let (mut app, _, _, _) = fixture();
            handle(&mut app, shift_key(KeyCode::Char('>'))).unwrap();
            let board = app.board.as_ref().unwrap();
            assert_eq!(board.lists[0].name, "B");
            assert_eq!(board.lists[1].name, "A");
            assert_eq!(board.selected_list, 1);
        });
    }

    #[test]
    fn next_matching_card_helper_returns_none_for_no_match() {
        let board = loaded_board(vec![fixed_card("c1", "alpha")]);
        assert!(next_matching_card(&board, 0, 0, "zzz").is_none());
    }

    #[test]
    fn t_starts_inline_title_edit() {
        with_temp_dir(|| {
            let (mut app, _, a_cards, _) = fixture();
            handle(&mut app, key(KeyCode::Char('t'))).unwrap();
            assert!(matches!(
                app.mode,
                AppMode::Insert(InsertTarget::EditCardTitleInline)
            ));
            // Title pre-filled
            assert_eq!(app.input_buffer, a_cards[0].title);
            // Return path: previous_mode set to Normal
            assert_eq!(app.previous_mode, Some(AppMode::Normal));
        });
    }

    #[test]
    fn e_starts_description_edit() {
        with_temp_dir(|| {
            let (mut app, meta, a_cards, _) = fixture();
            // Pre-populate description on selected card
            let cid = a_cards[0].id.clone();
            {
                let board = app.board.as_mut().unwrap();
                let card = board.cards.get_mut(&cid).unwrap();
                card.description = "hello desc".into();
                card_store::save_card(&meta.id, card).unwrap();
            }
            handle(&mut app, key(KeyCode::Char('e'))).unwrap();
            assert!(matches!(
                app.mode,
                AppMode::Insert(InsertTarget::EditCardDescription)
            ));
            // Description editor active with initial content
            let editor = app.description_editor.as_ref().expect("editor active");
            assert_eq!(editor.lines().join("\n"), "hello desc");
            // Return path: previous_mode set to Normal
            assert_eq!(app.previous_mode, Some(AppMode::Normal));
        });
    }

    #[test]
    fn visible_card_ids_skips_archived() {
        let mut cards = vec![fixed_card("id1", "a"), fixed_card("id2", "b")];
        cards[1].archived = true;
        let board = loaded_board(cards);
        assert_eq!(visible_card_ids(&board, 0), vec![0]);
    }
}
