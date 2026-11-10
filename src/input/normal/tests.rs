use super::*;
use crate::app::{App, AppMode};
use crate::model::board::BoardMeta;
use crate::model::card::Card;
use crate::storage::{board_store, card_store};
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

/// Three boards in display order, with the first one open.
fn three_boards_first_open() -> (App, Vec<BoardMeta>) {
    let metas: Vec<BoardMeta> = ["A", "B", "C"]
        .iter()
        .map(|n| board_store::create_board((*n).into()).unwrap())
        .collect();
    let app = App::new(Some(metas[0].id.clone())).unwrap();
    (app, metas)
}

fn open_board_id(app: &App) -> crate::model::id::BoardId {
    app.board().unwrap().meta.id.clone()
}

/// Build an App with a single list of cards with the given titles.
fn single_list_fixture(titles: &[&str]) -> (App, BoardMeta) {
    let mut meta = board_store::create_board("Board".into()).unwrap();
    let list = crate::test_support::add_list(&mut meta, "L");
    let mut cards: Vec<Card> = titles.iter().map(|t| Card::new((*t).into())).collect();
    crate::test_support::put_cards(&meta.id, &list, &mut cards);
    (App::new(Some(meta.id.clone())).unwrap(), meta)
}

/// Build an App with a board having two lists; list 0 has 3 cards, list 1 has 1.
fn fixture() -> (App, BoardMeta, Vec<Card>, Vec<Card>) {
    let mut meta = board_store::create_board("Board".into()).unwrap();
    let list_a = crate::test_support::add_list(&mut meta, "A");
    let list_b = crate::test_support::add_list(&mut meta, "B");

    let mut a_cards: Vec<Card> = (0..3).map(|i| Card::new(format!("a{i}"))).collect();
    let mut b_cards = vec![Card::new("b0".into())];
    crate::test_support::put_cards(&meta.id, &list_a, &mut a_cards);
    crate::test_support::put_cards(&meta.id, &list_b, &mut b_cards);

    let app = App::new(Some(meta.id.clone())).unwrap();
    (app, meta, a_cards, b_cards)
}

#[test]
fn down_moves_selection() {
    with_temp_dir(|| {
        let (mut app, _, _, _) = fixture();
        assert_eq!(app.board().unwrap().selected_card[0], 0);
        handle(&mut app, key(KeyCode::Down)).unwrap();
        assert_eq!(app.board().unwrap().selected_card[0], 1);
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
        assert_eq!(app.board().unwrap().selected_card[0], 2);
    });
}

#[test]
fn up_stops_at_zero() {
    with_temp_dir(|| {
        let (mut app, _, _, _) = fixture();
        for _ in 0..5 {
            handle(&mut app, key(KeyCode::Up)).unwrap();
        }
        assert_eq!(app.board().unwrap().selected_card[0], 0);
    });
}

#[test]
fn right_moves_to_next_list() {
    with_temp_dir(|| {
        let (mut app, _, _, _) = fixture();
        handle(&mut app, key(KeyCode::Right)).unwrap();
        assert_eq!(app.board().unwrap().selected_list, 1);
    });
}

#[test]
fn right_stops_at_last_list() {
    with_temp_dir(|| {
        let (mut app, _, _, _) = fixture();
        for _ in 0..5 {
            handle(&mut app, key(KeyCode::Right)).unwrap();
        }
        assert_eq!(app.board().unwrap().selected_list, 1);
    });
}

#[test]
fn left_stops_at_zero() {
    with_temp_dir(|| {
        let (mut app, _, _, _) = fixture();
        for _ in 0..5 {
            handle(&mut app, key(KeyCode::Left)).unwrap();
        }
        assert_eq!(app.board().unwrap().selected_list, 0);
    });
}

#[test]
fn g_jumps_to_top() {
    with_temp_dir(|| {
        let (mut app, _, _, _) = fixture();
        app.board_mut().unwrap().selected_card[0] = 2;
        handle(&mut app, key(KeyCode::Char('g'))).unwrap();
        assert_eq!(app.board().unwrap().selected_card[0], 0);
    });
}

#[test]
fn shift_g_jumps_to_bottom() {
    with_temp_dir(|| {
        let (mut app, _, _, _) = fixture();
        handle(&mut app, shift_key(KeyCode::Char('G'))).unwrap();
        assert_eq!(app.board().unwrap().selected_card[0], 2);
    });
}

#[test]
fn shift_down_moves_card_within_list() {
    with_temp_dir(|| {
        let (mut app, _, a_cards, _) = fixture();
        // Cards initially [a0, a1, a2]; selection at 0
        handle(&mut app, shift_key(KeyCode::Down)).unwrap();
        let board = app.board().unwrap();
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
        let board = app.board().unwrap();
        assert_eq!(board.lists[0].card_ids[0], a_cards[0].id);
        assert_eq!(board.selected_card[0], 0);
    });
}

#[test]
fn shift_down_at_bottom_is_noop() {
    with_temp_dir(|| {
        let (mut app, _, a_cards, _) = fixture();
        app.board_mut().unwrap().selected_card[0] = 2;
        handle(&mut app, shift_key(KeyCode::Down)).unwrap();
        let board = app.board().unwrap();
        assert_eq!(board.lists[0].card_ids[2], a_cards[2].id);
        assert_eq!(board.selected_card[0], 2);
    });
}

#[test]
fn shift_right_moves_card_to_next_list() {
    with_temp_dir(|| {
        let (mut app, _, a_cards, _) = fixture();
        handle(&mut app, shift_key(KeyCode::Right)).unwrap();
        let board = app.board().unwrap();
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
        let board = app.board().unwrap();
        assert_eq!(board.lists[0].card_ids.len(), 3);
        assert_eq!(board.lists[0].card_ids[0], a_cards[0].id);
    });
}

// Grab mode: `m` grabs the selected card, plain arrows move it — the
// only move path on terminals without modifier reporting on arrows
// (Linux console).

#[test]
fn grab_then_down_moves_card_within_list() {
    with_temp_dir(|| {
        let (mut app, _, a_cards, _) = fixture();
        handle(&mut app, key(KeyCode::Char('m'))).unwrap();
        assert!(app.grab_active);
        handle(&mut app, key(KeyCode::Down)).unwrap();
        let board = app.board().unwrap();
        assert_eq!(board.lists[0].card_ids[1], a_cards[0].id);
        assert_eq!(board.selected_card[0], 1);
        assert!(app.grab_active, "card stays grabbed across moves");
    });
}

#[test]
fn grab_then_right_moves_card_to_next_list() {
    with_temp_dir(|| {
        let (mut app, _, a_cards, _) = fixture();
        handle(&mut app, key(KeyCode::Char('m'))).unwrap();
        handle(&mut app, key(KeyCode::Right)).unwrap();
        let board = app.board().unwrap();
        assert_eq!(board.lists[0].card_ids.len(), 2);
        assert!(board.lists[1].card_ids.contains(&a_cards[0].id));
        assert_eq!(board.selected_list, 1);
    });
}

#[test]
fn esc_drops_grab_and_arrows_navigate_again() {
    with_temp_dir(|| {
        let (mut app, _, a_cards, _) = fixture();
        handle(&mut app, key(KeyCode::Char('m'))).unwrap();
        handle(&mut app, key(KeyCode::Esc)).unwrap();
        assert!(!app.grab_active);
        handle(&mut app, key(KeyCode::Down)).unwrap();
        let board = app.board().unwrap();
        // Selection moved, card order untouched.
        assert_eq!(board.lists[0].card_ids[0], a_cards[0].id);
        assert_eq!(board.selected_card[0], 1);
    });
}

#[test]
fn m_drops_grab_again() {
    with_temp_dir(|| {
        let (mut app, _, _, _) = fixture();
        handle(&mut app, key(KeyCode::Char('m'))).unwrap();
        handle(&mut app, key(KeyCode::Char('m'))).unwrap();
        assert!(!app.grab_active);
    });
}

#[test]
fn grab_swallows_other_keys() {
    with_temp_dir(|| {
        let (mut app, _, _, _) = fixture();
        handle(&mut app, key(KeyCode::Char('m'))).unwrap();
        handle(&mut app, key(KeyCode::Char('n'))).unwrap();
        assert!(app.grab_active);
        assert!(matches!(app.mode, AppMode::Normal));
        assert!(
            app.insert.is_none(),
            "grab must not open the new-card insert"
        );
    });
}

#[test]
fn grab_needs_a_selected_card() {
    with_temp_dir(|| {
        let (mut app, _) = single_list_fixture(&[]);
        handle(&mut app, key(KeyCode::Char('m'))).unwrap();
        assert!(!app.grab_active);
    });
}

#[test]
fn shift_right_from_last_list_is_noop() {
    with_temp_dir(|| {
        let (mut app, _, _, b_cards) = fixture();
        handle(&mut app, key(KeyCode::Right)).unwrap();
        assert_eq!(app.board().unwrap().selected_list, 1);
        handle(&mut app, shift_key(KeyCode::Right)).unwrap();
        let board = app.board().unwrap();
        assert_eq!(board.lists[1].card_ids.len(), 1);
        assert_eq!(board.lists[1].card_ids[0], b_cards[0].id);
    });
}

/// Full keypress path: Shift+Up on a visible card whose visible neighbor is
/// separated by archived (hidden) cards still moves it one visible slot.
/// Regression for migrated boards where archived cards interleave.
#[test]
fn shift_up_moves_past_interleaved_archived_cards() {
    with_temp_dir(|| {
        let mut meta = board_store::create_board("Board".into()).unwrap();
        let list = crate::test_support::add_list(&mut meta, "TODO");
        // top (visible), arch, arch, bottom (visible) — in card_ids order.
        let titles_archived = [
            ("top", false),
            ("a1", true),
            ("a2", true),
            ("bottom", false),
        ];
        let mut cards: Vec<Card> = titles_archived
            .iter()
            .map(|(t, archived)| {
                let mut c = Card::new((*t).into());
                c.archived = *archived;
                c
            })
            .collect();
        crate::test_support::put_cards(&meta.id, &list, &mut cards);
        let ids: Vec<_> = cards.iter().map(|c| c.id.clone()).collect();
        let mut app = App::new(Some(meta.id.clone())).unwrap();

        // Select the bottom visible card, then move it up.
        handle(&mut app, key(KeyCode::Down)).unwrap();
        let board = app.board().unwrap();
        assert_eq!(
            board.current_card_id(),
            Some(&ids[3]),
            "Down selects bottom visible card across the archived gap"
        );

        handle(&mut app, shift_key(KeyCode::Up)).unwrap();
        let board = app.board().unwrap();
        let visible: Vec<_> = board
            .visible_cards(0, None)
            .into_iter()
            .map(|i| board.lists[0].card_ids[i].clone())
            .collect();
        assert_eq!(
            visible,
            vec![ids[3].clone(), ids[0].clone()],
            "bottom moved above top"
        );
        assert_eq!(board.current_card_id(), Some(&ids[3]), "selection follows");
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
        assert!(app.editor.is_none());
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
fn c_and_d_toggle_the_date_stamps_independently_and_persist() {
    with_temp_dir(|| {
        let (mut app, meta, _, _) = fixture();
        let plain = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty());
        handle(&mut app, plain('c')).unwrap();
        let m = &app.board().unwrap().meta;
        assert!(m.show_created_date && !m.show_updated_date);
        handle(&mut app, plain('d')).unwrap();
        let m = &app.board().unwrap().meta;
        assert!(m.show_created_date && m.show_updated_date);
        let on_disk = board_store::load_board(&meta.id).unwrap();
        assert!(on_disk.show_created_date && on_disk.show_updated_date);
        handle(&mut app, plain('c')).unwrap();
        assert!(!app.board().unwrap().meta.show_created_date);
        assert!(app.board().unwrap().meta.show_updated_date);
    });
}

#[test]
fn shift_u_clears_due_date() {
    with_temp_dir(|| {
        let (mut app, meta, a_cards, _) = fixture();
        // Set a due date on selected card
        let cid = a_cards[0].id.clone();
        {
            let board = app.board_mut().unwrap();
            let card = board.cards.get_mut(&cid).unwrap();
            card.due_date = Some(chrono::NaiveDate::from_ymd_opt(2099, 1, 1).unwrap());
            card_store::save_card(&meta.id, card).unwrap();
        }
        handle(&mut app, shift_key(KeyCode::Char('U'))).unwrap();
        assert!(
            app.board()
                .unwrap()
                .cards
                .get(&cid)
                .unwrap()
                .due_date
                .is_some()
        );
        crate::input::handle_input(
            &mut app,
            KeyEvent::new(KeyCode::Char('y'), KeyModifiers::empty()),
        )
        .unwrap();
        let board = app.board().unwrap();
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
        assert_eq!(app.board().unwrap().selected_card[0], 1);
        // From index 1, no further match → stays at 1
        handle(&mut app, key(KeyCode::Down)).unwrap();
        assert_eq!(app.board().unwrap().selected_card[0], 1);
    });
}

#[test]
fn search_nav_up_finds_previous_match() {
    with_temp_dir(|| {
        let (mut app, _) = single_list_fixture(&["BINGO one", "nope", "BINGO three"]);
        app.search_active = true;
        app.search_query = "BINGO".into();
        app.board_mut().unwrap().selected_card[0] = 2;
        handle(&mut app, key(KeyCode::Up)).unwrap();
        assert_eq!(app.board().unwrap().selected_card[0], 0);
    });
}

#[test]
fn shift_left_arrow_swaps_lists() {
    with_temp_dir(|| {
        let (mut app, _, _, _) = fixture();
        handle(&mut app, key(KeyCode::Right)).unwrap(); // move to list 1
        handle(&mut app, shift_key(KeyCode::Char('<'))).unwrap();
        let board = app.board().unwrap();
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
        let board = app.board().unwrap();
        assert_eq!(board.lists[0].name, "B");
        assert_eq!(board.lists[1].name, "A");
        assert_eq!(board.selected_list, 1);
    });
}

#[test]
fn t_starts_inline_title_edit() {
    with_temp_dir(|| {
        let (mut app, _, a_cards, _) = fixture();
        handle(&mut app, key(KeyCode::Char('t'))).unwrap();
        assert!(matches!(app.mode, AppMode::Insert));
        // Title pre-filled in the handler's buffer
        let h = app.insert.as_ref().unwrap();
        assert_eq!(h.line_buffer(), Some(a_cards[0].title.as_str()));
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
            let board = app.board_mut().unwrap();
            let card = board.cards.get_mut(&cid).unwrap();
            card.description = "hello desc".into();
            card_store::save_card(&meta.id, card).unwrap();
        }
        handle(&mut app, key(KeyCode::Char('e'))).unwrap();
        assert!(matches!(app.mode, AppMode::Insert));
        let editor = app
            .insert
            .as_ref()
            .unwrap()
            .as_any()
            .downcast_ref::<crate::insert::markdown_editor::MarkdownEditor>()
            .expect("description editor active");
        assert_eq!(editor.input.current_text(), "hello desc");
        assert_eq!(app.previous_mode, Some(AppMode::Normal));
    });
}

#[test]
fn k_switches_to_next_board() {
    with_temp_dir(|| {
        let (mut app, metas) = three_boards_first_open();
        assert_eq!(open_board_id(&app), metas[0].id);
        handle(&mut app, key(KeyCode::Char('k'))).unwrap();
        assert_eq!(open_board_id(&app), metas[1].id);
        assert_eq!(app.selected_board_idx, 1);
        assert_eq!(app.mode, AppMode::Normal);
    });
}

#[test]
fn j_wraps_to_last_board() {
    with_temp_dir(|| {
        let (mut app, metas) = three_boards_first_open();
        handle(&mut app, key(KeyCode::Char('j'))).unwrap();
        assert_eq!(open_board_id(&app), metas[2].id);
        assert_eq!(app.selected_board_idx, 2);
    });
}

#[test]
fn k_wraps_from_last_to_first() {
    with_temp_dir(|| {
        let (mut app, metas) = three_boards_first_open();
        app.load_board(&metas[2].id).unwrap();
        app.selected_board_idx = 2;
        handle(&mut app, key(KeyCode::Char('k'))).unwrap();
        assert_eq!(open_board_id(&app), metas[0].id);
        assert_eq!(app.selected_board_idx, 0);
    });
}

#[test]
fn comma_and_dot_mirror_j_and_k() {
    with_temp_dir(|| {
        let (mut app, metas) = three_boards_first_open();
        // '.' = next
        handle(&mut app, key(KeyCode::Char('.'))).unwrap();
        assert_eq!(open_board_id(&app), metas[1].id);
        // ',' = previous
        handle(&mut app, key(KeyCode::Char(','))).unwrap();
        assert_eq!(open_board_id(&app), metas[0].id);
        // ',' wraps to last
        handle(&mut app, key(KeyCode::Char(','))).unwrap();
        assert_eq!(open_board_id(&app), metas[2].id);
    });
}

#[test]
fn j_k_are_noop_with_single_board() {
    with_temp_dir(|| {
        let m = board_store::create_board("Solo".into()).unwrap();
        let mut app = App::new(Some(m.id.clone())).unwrap();
        handle(&mut app, key(KeyCode::Char('j'))).unwrap();
        assert_eq!(open_board_id(&app), m.id);
        handle(&mut app, key(KeyCode::Char('k'))).unwrap();
        assert_eq!(open_board_id(&app), m.id);
    });
}

#[test]
fn switching_boards_via_k_j_remembers_and_restores_focused_card() {
    with_temp_dir(|| {
        let mut meta_a = board_store::create_board("Board A".into()).unwrap();
        let list_a = crate::test_support::add_list(&mut meta_a, "List A");
        let mut cards_a = vec![Card::new("A1".into()), Card::new("A2".into())];
        crate::test_support::put_cards(&meta_a.id, &list_a, &mut cards_a);

        let mut meta_b = board_store::create_board("Board B".into()).unwrap();
        let list_b = crate::test_support::add_list(&mut meta_b, "List B");
        let mut cards_b = vec![
            Card::new("B1".into()),
            Card::new("B2".into()),
            Card::new("B3".into()),
        ];
        crate::test_support::put_cards(&meta_b.id, &list_b, &mut cards_b);

        let mut app = App::new(Some(meta_a.id.clone())).unwrap();
        assert_eq!(open_board_id(&app), meta_a.id);
        assert_eq!(
            app.board().unwrap().current_card_id().unwrap(),
            &cards_a[0].id
        );

        // Move down to focus A2
        handle(&mut app, key(KeyCode::Down)).unwrap();
        assert_eq!(
            app.board().unwrap().current_card_id().unwrap(),
            &cards_a[1].id
        );

        // Switch to Board B with 'k'
        handle(&mut app, key(KeyCode::Char('k'))).unwrap();
        assert_eq!(open_board_id(&app), meta_b.id);
        assert_eq!(
            app.board().unwrap().current_card_id().unwrap(),
            &cards_b[0].id
        );

        // Move down twice on Board B to focus B3
        handle(&mut app, key(KeyCode::Down)).unwrap();
        handle(&mut app, key(KeyCode::Down)).unwrap();
        assert_eq!(
            app.board().unwrap().current_card_id().unwrap(),
            &cards_b[2].id
        );

        // Switch back to Board A with 'j' — should restore A2 as focused
        handle(&mut app, key(KeyCode::Char('j'))).unwrap();
        assert_eq!(open_board_id(&app), meta_a.id);
        assert_eq!(
            app.board().unwrap().current_card_id().unwrap(),
            &cards_a[1].id
        );

        // Switch back to Board B with 'k' — should restore B3 as focused
        handle(&mut app, key(KeyCode::Char('k'))).unwrap();
        assert_eq!(open_board_id(&app), meta_b.id);
        assert_eq!(
            app.board().unwrap().current_card_id().unwrap(),
            &cards_b[2].id
        );
    });
}

#[test]
fn switching_boards_remembers_and_restores_scroll_offset() {
    with_temp_dir(|| {
        let mut meta_a = board_store::create_board("Board A".into()).unwrap();
        let list_a = crate::test_support::add_list(&mut meta_a, "List A");
        let mut cards_a: Vec<Card> = (1..=15).map(|i| Card::new(format!("A{i:02}"))).collect();
        crate::test_support::put_cards(&meta_a.id, &list_a, &mut cards_a);

        let mut meta_b = board_store::create_board("Board B".into()).unwrap();
        let list_b = crate::test_support::add_list(&mut meta_b, "List B");
        let mut cards_b = vec![Card::new("B1".into())];
        crate::test_support::put_cards(&meta_b.id, &list_b, &mut cards_b);

        let mut app = App::new(Some(meta_a.id.clone())).unwrap();
        let backend = ratatui::backend::TestBackend::new(100, 30);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();

        // Render Board A initially
        terminal.draw(|f| crate::ui::render(f, &app)).unwrap();
        assert_eq!(app.board().unwrap().scroll_offset[0].get(), 0);

        // Navigate down to Card 15 (bottom)
        handle(&mut app, key(KeyCode::Char('G'))).unwrap();
        terminal.draw(|f| crate::ui::render(f, &app)).unwrap();
        let scroll_a = app.board().unwrap().scroll_offset[0].get();
        assert!(scroll_a > 0, "expected scrolled list, got {scroll_a}");

        // Move up one card: focus on A14, scroll offset unchanged
        handle(&mut app, key(KeyCode::Up)).unwrap();
        terminal.draw(|f| crate::ui::render(f, &app)).unwrap();
        assert_eq!(app.board().unwrap().scroll_offset[0].get(), scroll_a);
        assert_eq!(
            app.board().unwrap().current_card_id().unwrap(),
            &cards_a[13].id
        );

        // Switch to Board B with 'k'
        handle(&mut app, key(KeyCode::Char('k'))).unwrap();
        assert_eq!(open_board_id(&app), meta_b.id);
        terminal.draw(|f| crate::ui::render(f, &app)).unwrap();
        assert_eq!(app.board().unwrap().scroll_offset[0].get(), 0);

        // Switch back to Board A with 'j'
        handle(&mut app, key(KeyCode::Char('j'))).unwrap();
        assert_eq!(open_board_id(&app), meta_a.id);
        assert_eq!(
            app.board().unwrap().current_card_id().unwrap(),
            &cards_a[13].id
        );
        assert_eq!(app.board().unwrap().scroll_offset[0].get(), scroll_a);

        // Press Up on Board A: moves focus up, does not scroll viewport
        handle(&mut app, key(KeyCode::Up)).unwrap();
        terminal.draw(|f| crate::ui::render(f, &app)).unwrap();
        assert_eq!(
            app.board().unwrap().current_card_id().unwrap(),
            &cards_a[12].id
        );
        assert_eq!(app.board().unwrap().scroll_offset[0].get(), scroll_a);
    });
}

#[test]
fn switching_boards_via_board_selector_remembers_and_restores_focused_card() {
    with_temp_dir(|| {
        let mut meta_a = board_store::create_board("Board A".into()).unwrap();
        let list_a = crate::test_support::add_list(&mut meta_a, "List A");
        let mut cards_a = vec![Card::new("A1".into()), Card::new("A2".into())];
        crate::test_support::put_cards(&meta_a.id, &list_a, &mut cards_a);

        let mut meta_b = board_store::create_board("Board B".into()).unwrap();
        let list_b = crate::test_support::add_list(&mut meta_b, "List B");
        let mut cards_b = vec![Card::new("B1".into()), Card::new("B2".into())];
        crate::test_support::put_cards(&meta_b.id, &list_b, &mut cards_b);

        let mut app = App::new(Some(meta_a.id.clone())).unwrap();
        // Focus A2 on Board A
        handle(&mut app, key(KeyCode::Down)).unwrap();
        assert_eq!(
            app.board().unwrap().current_card_id().unwrap(),
            &cards_a[1].id
        );

        // Close Board A to return to selector
        handle(&mut app, key(KeyCode::Char('b'))).unwrap();
        assert_eq!(app.mode, AppMode::BoardSelector);

        // Select Board B and open it
        app.selected_board_idx = 1;
        crate::input::board_selector_input::handle(&mut app, key(KeyCode::Enter)).unwrap();
        assert_eq!(open_board_id(&app), meta_b.id);
        assert_eq!(
            app.board().unwrap().current_card_id().unwrap(),
            &cards_b[0].id
        );

        // Focus B2 on Board B
        handle(&mut app, key(KeyCode::Down)).unwrap();
        assert_eq!(
            app.board().unwrap().current_card_id().unwrap(),
            &cards_b[1].id
        );

        // Close Board B to selector
        handle(&mut app, key(KeyCode::Char('b'))).unwrap();
        assert_eq!(app.mode, AppMode::BoardSelector);

        // Re-open Board A
        app.selected_board_idx = 0;
        crate::input::board_selector_input::handle(&mut app, key(KeyCode::Enter)).unwrap();
        assert_eq!(open_board_id(&app), meta_a.id);
        assert_eq!(
            app.board().unwrap().current_card_id().unwrap(),
            &cards_a[1].id
        );

        // Close Board A and re-open Board B
        handle(&mut app, key(KeyCode::Char('b'))).unwrap();
        app.selected_board_idx = 1;
        crate::input::board_selector_input::handle(&mut app, key(KeyCode::Enter)).unwrap();
        assert_eq!(open_board_id(&app), meta_b.id);
        assert_eq!(
            app.board().unwrap().current_card_id().unwrap(),
            &cards_b[1].id
        );
    });
}

#[test]
fn restoring_focus_falls_back_when_remembered_card_deleted() {
    with_temp_dir(|| {
        let mut meta_a = board_store::create_board("Board A".into()).unwrap();
        let list_a = crate::test_support::add_list(&mut meta_a, "List A");
        let mut cards_a = vec![Card::new("A1".into()), Card::new("A2".into())];
        crate::test_support::put_cards(&meta_a.id, &list_a, &mut cards_a);

        let mut meta_b = board_store::create_board("Board B".into()).unwrap();
        let list_b = crate::test_support::add_list(&mut meta_b, "List B");
        let mut cards_b = vec![Card::new("B1".into())];
        crate::test_support::put_cards(&meta_b.id, &list_b, &mut cards_b);

        let mut app = App::new(Some(meta_a.id.clone())).unwrap();
        // Focus A2
        handle(&mut app, key(KeyCode::Down)).unwrap();
        assert_eq!(
            app.board().unwrap().current_card_id().unwrap(),
            &cards_a[1].id
        );

        // Switch to Board B
        handle(&mut app, key(KeyCode::Char('k'))).unwrap();
        assert_eq!(open_board_id(&app), meta_b.id);

        // Delete card A2 from disk while on Board B
        crate::storage::card_store::delete_card(&meta_a.id, &cards_a[1].id).unwrap();

        // Switch back to Board A
        handle(&mut app, key(KeyCode::Char('j'))).unwrap();
        assert_eq!(open_board_id(&app), meta_a.id);
        // Gracefully falls back to A1
        assert_eq!(
            app.board().unwrap().current_card_id().unwrap(),
            &cards_a[0].id
        );
    });
}

#[test]
fn unarchive_card_requires_dialog_confirmation() {
    with_temp_dir(|| {
        let (mut app, _, a_cards, _) = fixture();
        let card_id = a_cards[0].id.clone();

        // Archive the first card: 'a' opens ConfirmArchiveCard, 'y' confirms
        handle(&mut app, key(KeyCode::Char('a'))).unwrap();
        assert!(app.dialog.is_some());
        crate::input::handle_input(&mut app, key(KeyCode::Char('y'))).unwrap();
        assert_eq!(app.mode, AppMode::Normal);
        assert!(app.board().unwrap().cards.get(&card_id).unwrap().archived);

        // Open archived cards dialog
        handle(&mut app, key(KeyCode::Char('v'))).unwrap();
        assert_eq!(app.mode, AppMode::Dialog);

        // Press Enter to restore: should open confirmation dialog instead of restoring immediately
        crate::input::handle_input(&mut app, key(KeyCode::Enter)).unwrap();
        assert_eq!(app.mode, AppMode::Dialog);
        // Cancel with 'n'
        crate::input::handle_input(&mut app, key(KeyCode::Char('n'))).unwrap();
        assert_eq!(app.mode, AppMode::Dialog);
        assert!(app.board().unwrap().cards.get(&card_id).unwrap().archived);

        // Press Enter then 'y' to confirm restore
        crate::input::handle_input(&mut app, key(KeyCode::Enter)).unwrap();
        crate::input::handle_input(&mut app, key(KeyCode::Char('y'))).unwrap();
        assert_eq!(app.mode, AppMode::Normal);
        assert!(!app.board().unwrap().cards.get(&card_id).unwrap().archived);
    });
}

#[test]
fn unarchive_list_requires_dialog_confirmation() {
    with_temp_dir(|| {
        let (mut app, _, _, _) = fixture();
        let list_id = app.board().unwrap().lists[0].id.clone();

        // Archive the first list: 'A' opens ConfirmArchiveList, 'y' confirms
        handle(&mut app, key(KeyCode::Char('A'))).unwrap();
        assert!(app.dialog.is_some());
        crate::input::handle_input(&mut app, key(KeyCode::Char('y'))).unwrap();
        assert_eq!(app.mode, AppMode::Normal);
        assert!(app.board().unwrap().lists.iter().all(|l| l.id != list_id));

        // Open archived lists dialog
        handle(&mut app, key(KeyCode::Char('V'))).unwrap();
        assert_eq!(app.mode, AppMode::Dialog);

        // Press Enter to restore: should open confirmation dialog instead of restoring immediately
        crate::input::handle_input(&mut app, key(KeyCode::Enter)).unwrap();
        assert_eq!(app.mode, AppMode::Dialog);
        // Cancel with 'n'
        crate::input::handle_input(&mut app, key(KeyCode::Char('n'))).unwrap();
        assert_eq!(app.mode, AppMode::Dialog);
        assert!(app.board().unwrap().lists.iter().all(|l| l.id != list_id));

        // Press Enter then 'y' to confirm restore
        crate::input::handle_input(&mut app, key(KeyCode::Enter)).unwrap();
        crate::input::handle_input(&mut app, key(KeyCode::Char('y'))).unwrap();
        assert_eq!(app.mode, AppMode::Normal);
        assert!(app.board().unwrap().lists.iter().any(|l| l.id == list_id));
    });
}

#[test]
fn capital_c_commits_to_git_or_toasts_why_not() {
    with_temp_dir(|| {
        // Not a git repository: the reason shows as a toast.
        let meta = board_store::create_board("B".into()).unwrap();
        let mut app = App::new(Some(meta.id)).unwrap();
        handle(&mut app, key(KeyCode::Char('C'))).unwrap();
        let (msg, _) = app.status_message.clone().unwrap();
        assert!(msg.starts_with("cannot commit: "), "{msg}");
    });
}
