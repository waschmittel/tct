use super::*;

#[test]
fn snapshot_board_selector() {
    with_temp_dir(|| {
        seed_selector_boards();
        let app = App::new(None).unwrap();
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_board_view() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let app = App::new(Some(id)).unwrap();
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_board_view_commit_outside_git() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Char('C'));
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_board_view_search_active() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Char('/'));
        for ch in "dash".chars() {
            press(&mut app, KeyCode::Char(ch));
        }
        press(&mut app, KeyCode::Enter);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_board_view_search_typing() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Char('/'));
        for ch in "token".chars() {
            press(&mut app, KeyCode::Char(ch));
        }
        assert_eq!(app.mode, AppMode::Command);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_status_toast() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        app.set_status("Archived card 'Fix login flow'".into());
        insta::assert_snapshot!(render_to_string(&app));
    });
}

/// Grab state: the status-bar chip flips to MOVE while `m` has a card
/// grabbed (arrow keys move it instead of navigating).
#[test]
fn snapshot_board_view_grab_mode() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Char('m'));
        assert!(app.grab_active);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

/// Degraded tier (Linux console): the selected card gets a double-line
/// border instead of the heavy one — visible in the text golden.
#[test]
fn snapshot_board_view_linux_tty() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        app.caps = crate::term_caps::TermCaps::linux_tty();
        insta::assert_snapshot!(render_to_string(&app));
    });
}

/// Degraded tier: checklist marks render as `x` (✓ is missing from
/// console fonts).
#[test]
fn snapshot_card_detail_linux_tty() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        app.caps = crate::term_caps::TermCaps::linux_tty();
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

/// On the bottom tier no RGB color may survive to the terminal (the Linux
/// console collapses them to unreadable black/white), and the selected
/// card must keep a visible background. Style-level check — the text
/// goldens above can't see colors.
#[test]
fn linux_tty_render_has_no_rgb_and_visible_selection() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        app.caps = crate::term_caps::TermCaps::linux_tty();

        let backend = TestBackend::new(WIDTH, HEIGHT);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| crate::ui::render(frame, &app))
            .unwrap();
        let buffer = terminal.backend().buffer();

        use ratatui::style::Color;
        let mut selection_bg_cells = 0;
        for cell in buffer.content.iter() {
            assert!(
                !matches!(cell.fg, Color::Rgb(..)) && !matches!(cell.bg, Color::Rgb(..)),
                "RGB color leaked into a degraded frame: {cell:?}"
            );
            if cell.bg == app.caps.selection_bg() {
                selection_bg_cells += 1;
            }
        }
        assert!(
            selection_bg_cells > 0,
            "selected card background not visible"
        );
    });
}

#[test]
fn snapshot_board_view_empty() {
    with_temp_dir(|| {
        let meta = crate::model::board::BoardMeta::new("Empty".into());
        board_store::save_board(&meta).unwrap();
        let app = App::new(Some(meta.id.clone())).unwrap();
        insta::assert_snapshot!(render_to_string(&app));
    });
}
