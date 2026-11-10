use super::*;

#[test]
fn snapshot_description_editor_scrollbar() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        set_long_description(&mut app);
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char('e'));
        assert_eq!(app.mode, AppMode::Insert);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_description_editor() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        {
            let board = app.board_mut().unwrap();
            let card_id = board.current_card_id().cloned().unwrap();
            let card = board.cards.get_mut(&card_id).unwrap();
            card.description = "Token expires too early.\n\n- check refresh\n- add test\n\n> Note: verify with backend team".into();
        }
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        press(&mut app, KeyCode::Char('e'));
        assert_eq!(app.mode, AppMode::Insert);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_description_editor_enter_citation() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        {
            let board = app.board_mut().unwrap();
            let card_id = board.current_card_id().cloned().unwrap();
            let card = board.cards.get_mut(&card_id).unwrap();
            card.description = "> First citation line".into();
        }
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        press(&mut app, KeyCode::Char('e'));
        assert_eq!(app.mode, AppMode::Insert);
        press(&mut app, KeyCode::Enter);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_description_editor_paste() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        press(&mut app, KeyCode::Char('e'));
        assert_eq!(app.mode, AppMode::Insert);
        crate::input::handle_paste(&mut app, "\n- pasted item".to_string()).unwrap();
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_description_editor_paste_mode() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char('e'));
        assert_eq!(app.mode, AppMode::Insert);
        crate::input::handle_input(
            &mut app,
            KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
        )
        .unwrap();
        crate::input::handle_paste(&mut app, "\n1. a\n7. b".to_string()).unwrap();
        insta::assert_snapshot!(render_to_string(&app));
    });
}

/// Widget-level golden for the date-picker popup (layout broke twice:
/// oversized popup, off-center calendar). Rendered with a fixed date so
/// the text is deterministic — the today-highlight is style-only and
/// invisible in the text golden.
#[test]
fn snapshot_date_picker() {
    let backend = TestBackend::new(40, 14);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            crate::ui::widgets::date_picker::render(
                frame,
                frame.area(),
                "2026-05-18",
                10,
                chrono::NaiveDate::from_ymd_opt(2026, 5, 18),
                ratatui::style::Color::Cyan,
            );
        })
        .unwrap();
    insta::assert_snapshot!(buffer_to_string(terminal.backend().buffer()));
}
