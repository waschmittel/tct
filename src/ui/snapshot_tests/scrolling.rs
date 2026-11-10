use super::*;

/// A list taller than the viewport carries a scroll thumb on its right
/// border; scrolling to the end moves it to the bottom.
#[test]
fn snapshot_long_list_scrollbar_top() {
    with_temp_dir(|| {
        let id = seed_long_list_board();
        let app = App::new(Some(id)).unwrap();
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_long_list_scrollbar_bottom() {
    with_temp_dir(|| {
        let id = seed_long_list_board();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Char('G'));
        insta::assert_snapshot!(render_to_string(&app));
    });
}

/// Scrolling up from the bottom of a long list moves focus up within the visible
/// window first without scrolling the viewport.
#[test]
fn snapshot_long_list_scroll_up_moves_focus_not_viewport() {
    with_temp_dir(|| {
        let id = seed_long_list_board();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Char('G'));
        press(&mut app, KeyCode::Up);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

/// Continuing to scroll up moves focus up to the top of the visible window
/// while the viewport still remains unchanged.
#[test]
fn snapshot_long_list_scroll_up_focus_at_top_of_window() {
    with_temp_dir(|| {
        let id = seed_long_list_board();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Char('G'));
        for _ in 0..7 {
            press(&mut app, KeyCode::Up);
        }
        insta::assert_snapshot!(render_to_string(&app));
    });
}

/// Once focus reaches the top of the visible window, pressing Up scrolls the
/// list up by one card.
#[test]
fn snapshot_long_list_scroll_up_scrolls_one_item() {
    with_temp_dir(|| {
        let id = seed_long_list_board();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Char('G'));
        for _ in 0..8 {
            press(&mut app, KeyCode::Up);
        }
        insta::assert_snapshot!(render_to_string(&app));
    });
}

/// Scrolling down in a long list: focus moves to the bottom of the window first,
/// then the list scrolls down one item.
#[test]
fn snapshot_long_list_scroll_down_one_item() {
    with_temp_dir(|| {
        let id = seed_long_list_board();
        let mut app = App::new(Some(id)).unwrap();
        for _ in 0..7 {
            press(&mut app, KeyCode::Down);
        }
        press(&mut app, KeyCode::Down);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn long_list_scrolling_down_and_up_step_by_step() {
    with_temp_dir(|| {
        let id = seed_long_list_board();
        let mut app = App::new(Some(id)).unwrap();
        let _ = render_to_string(&app);

        // Initially at top: selected card 0, scroll offset 0
        assert_eq!(app.board().unwrap().selected_card[0], 0);
        assert_eq!(app.board().unwrap().scroll_offset[0].get(), 0);

        // Moving down 1..=7 moves focus only, scroll offset stays 0
        for i in 1..=7 {
            press(&mut app, KeyCode::Down);
            assert_eq!(app.board().unwrap().selected_card[0], i);
            assert_eq!(app.board().unwrap().scroll_offset[0].get(), 0);
        }

        // 8th down press scrolls down by 1: selected 8, scroll 1
        press(&mut app, KeyCode::Down);
        assert_eq!(app.board().unwrap().selected_card[0], 8);
        assert_eq!(app.board().unwrap().scroll_offset[0].get(), 1);

        // Continue scrolling down to the end (cards 9, 10, 11)
        for (card_idx, expected_scroll) in [(9, 2), (10, 3), (11, 4)] {
            press(&mut app, KeyCode::Down);
            assert_eq!(app.board().unwrap().selected_card[0], card_idx);
            assert_eq!(app.board().unwrap().scroll_offset[0].get(), expected_scroll);
        }

        // Further Down does not scroll past end
        press(&mut app, KeyCode::Down);
        assert_eq!(app.board().unwrap().selected_card[0], 11);
        assert_eq!(app.board().unwrap().scroll_offset[0].get(), 4);

        // Moving UP:
        // Steps 1..=7 move focus up from 10 down to 4 (Card 05), scroll offset stays 4!
        for expected_card in (4..=10).rev() {
            press(&mut app, KeyCode::Up);
            assert_eq!(
                app.board().unwrap().selected_card[0],
                expected_card,
                "focus should move up"
            );
            assert_eq!(
                app.board().unwrap().scroll_offset[0].get(),
                4,
                "viewport should not scroll while focus moves up within window"
            );
        }

        // 8th Up press reaches beyond the visible window: selected becomes 3 (Card 04),
        // scroll offset scrolls up to 3!
        press(&mut app, KeyCode::Up);
        assert_eq!(app.board().unwrap().selected_card[0], 3);
        assert_eq!(app.board().unwrap().scroll_offset[0].get(), 3);

        // Continuing Up scrolls one item at a time:
        for (expected_card, expected_scroll) in [(2, 2), (1, 1), (0, 0)] {
            press(&mut app, KeyCode::Up);
            assert_eq!(app.board().unwrap().selected_card[0], expected_card);
            assert_eq!(app.board().unwrap().scroll_offset[0].get(), expected_scroll);
        }

        // Further Up does not scroll past top
        press(&mut app, KeyCode::Up);
        assert_eq!(app.board().unwrap().selected_card[0], 0);
        assert_eq!(app.board().unwrap().scroll_offset[0].get(), 0);
    });
}

#[test]
fn lists_that_fit_have_no_scroll_thumb() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let app = App::new(Some(id)).unwrap();
        assert!(thumb_rows(&render_to_string(&app)).is_empty());
    });
}

#[test]
fn scroll_thumb_moves_down_with_the_scroll_offset() {
    with_temp_dir(|| {
        let id = seed_long_list_board();
        let mut app = App::new(Some(id)).unwrap();
        let at_top = thumb_rows(&render_to_string(&app));
        press(&mut app, KeyCode::Char('G'));
        let at_bottom = thumb_rows(&render_to_string(&app));
        // Track: from under the list's top corner (row 1) to above its
        // bottom corner (row HEIGHT - 2).
        let (first_row, last_row) = (2, HEIGHT as usize - 3);
        assert_eq!(at_top.first(), Some(&first_row));
        assert!(at_top.last() < Some(&last_row));
        assert!(at_bottom.first() > at_top.first());
        assert_eq!(at_bottom.last(), Some(&last_row));
    });
}

#[test]
fn scroll_thumb_uses_accent_on_the_selected_list_and_gray_on_others() {
    with_temp_dir(|| {
        let mut meta = crate::model::board::BoardMeta::new("Two".into());
        board_store::save_board(&meta).unwrap();
        let long = crate::test_support::add_list(&mut meta, "Long");
        let other = crate::test_support::add_list(&mut meta, "Other");
        let mut cards: Vec<Card> = (1..=12)
            .map(|i| Card::new(format!("Card {i:02}")))
            .collect();
        crate::test_support::put_cards(&meta.id, &long, &mut cards);
        let mut more: Vec<Card> = (1..=12)
            .map(|i| Card::new(format!("Other {i:02}")))
            .collect();
        crate::test_support::put_cards(&meta.id, &other, &mut more);

        let app = App::new(Some(meta.id.clone())).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(WIDTH, HEIGHT)).unwrap();
        terminal.draw(|f| crate::ui::render(f, &app)).unwrap();
        let buf = terminal.backend().buffer();
        let thumb_fg = |x0: u16, x1: u16| {
            (0..HEIGHT)
                .flat_map(|y| (x0..x1).map(move |x| (x, y)))
                .find(|&(x, y)| x == x1 - 1 && buf[(x, y)].symbol() == "▌")
                .map(|(x, y)| buf[(x, y)].fg)
        };
        assert_eq!(thumb_fg(0, WIDTH / 2), Some(app.accent_color()));
        assert_eq!(
            thumb_fg(WIDTH / 2, WIDTH),
            Some(ratatui::style::Color::Gray)
        );
    });
}
