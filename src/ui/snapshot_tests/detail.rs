use super::*;

#[test]
fn snapshot_card_detail() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

/// Empty sections are hidden — except when every section is empty, all
/// four show with their placeholders (discoverability).
#[test]
fn snapshot_card_detail_all_sections_empty() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Down); // "Redesign dashboard": no content at all
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_card_detail_long_description_scrollbar() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        set_long_description(&mut app);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        // The renderer reports the description's max scroll; draw once so
        // the scroll keys have a bound (as in the real render loop).
        let _ = render_to_string(&app);
        press(&mut app, KeyCode::PageDown);
        press(&mut app, KeyCode::PageDown);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_card_detail_scrollbar_at_bottom() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        set_long_description(&mut app);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        // The renderer reports the description's max scroll; draw once so
        // the scroll keys have a bound (as in the real render loop).
        let _ = render_to_string(&app);
        // Scroll well past the end; input clamps at max scroll. The thumb
        // must sit at the very bottom of the track.
        for _ in 0..80 {
            press(&mut app, KeyCode::PageDown);
        }
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_card_detail_scrollbar_scroll_up_after_overshoot() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        set_long_description(&mut app);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        let _ = render_to_string(&app);
        for _ in 0..80 {
            press(&mut app, KeyCode::PageDown);
        }
        // Pressing PageUp immediately drops 1 row from max scroll
        press(&mut app, KeyCode::PageUp);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

/// Labels wrap at chip boundaries in the list view, and the info line
/// (due/checklist/description markers) keeps its row below them.
#[test]
fn snapshot_board_view_many_labels_wrap() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        add_many_labels(&mut app);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

/// Detail view shows label chips side by side, wrapped like on the card.
#[test]
fn snapshot_card_detail_many_labels_wrap() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        add_many_labels(&mut app);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

/// Long checklist items wrap; continuation lines align under the item text.
#[test]
fn snapshot_card_detail_long_checklist_item_wraps() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        {
            let board = app.board_mut().unwrap();
            let card_id = board.current_card_id().cloned().unwrap();
            let card = board.cards.get_mut(&card_id).unwrap();
            card.checklist[1].text =
                "Write a failing test that reproduces the premature token expiry \
                 on refresh, including the clock-skew edge case"
                    .into();
        }
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_card_detail_long_checklist_scrolled_bottom() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        set_long_checklist(&mut app);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        for _ in 0..15 {
            press(&mut app, KeyCode::Down);
        }
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_card_detail_long_checklist_scroll_up() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        set_long_checklist(&mut app);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        for _ in 0..15 {
            press(&mut app, KeyCode::Down);
        }
        // Navigate back up
        for _ in 0..3 {
            press(&mut app, KeyCode::Up);
        }
        insta::assert_snapshot!(render_to_string(&app));
    });
}

/// The popup stops growing once the description hits WRAP_WIDTH — on a
/// wide terminal it stays at wrap width + padding instead of 80% of the
/// screen.
#[test]
fn snapshot_card_detail_wide_terminal_capped() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        insta::assert_snapshot!(render_to_string_sized(&app, 160, 30));
    });
}

/// The date stamps sit in the card-detail frame's top/bottom right corners
/// in the accent color. Dates are pinned to a past year so the text is
/// deterministic (no snapshot: fixtures must not carry timestamps).
#[test]
fn card_detail_shows_date_stamps_in_accent_color() {
    use chrono::TimeZone;
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        let card_id = app.board().unwrap().current_card_id().unwrap().clone();
        {
            let card = app.board_mut().unwrap().cards.get_mut(&card_id).unwrap();
            card.created_at = chrono::Utc.with_ymd_and_hms(2020, 1, 2, 12, 0, 0).unwrap();
            card.history.clear();
            card.log("Edited");
            card.history[0].at = chrono::Utc.with_ymd_and_hms(2021, 3, 4, 12, 0, 0).unwrap();
        }
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        assert!(!render_to_string(&app).contains("2020-01-02"));

        press(&mut app, KeyCode::Char('c'));
        press(&mut app, KeyCode::Char('d'));
        let text = render_to_string(&app);
        let rows: Vec<&str> = text.lines().collect();
        let top = rows
            .iter()
            .find(|r| r.contains("┌ Fix login flow"))
            .expect("popup top");
        assert!(top.contains("2021-03-04"), "{text}");
        // The board card behind the popup carries a stamp too: the popup's is lowest.
        let bottom = rows
            .iter()
            .rev()
            .find(|r| r.contains("2020-01-02"))
            .expect("created stamp");
        assert!(
            top.contains(" 2021-03-04 ┐") && bottom.contains(" 2020-01-02 ┘"),
            "{text}"
        );

        let backend = TestBackend::new(WIDTH, HEIGHT);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| crate::ui::render(frame, &app))
            .unwrap();
        let buf = terminal.backend().buffer();
        let y = rows.iter().rposition(|r| r.contains("2020-01-02")).unwrap() as u16;
        let x = rows[y as usize]
            .find("2020-01-02")
            .map(|b| rows[y as usize][..b].chars().count())
            .unwrap() as u16;
        assert_eq!(buf[(x, y)].fg, app.accent_color());
    });
}
