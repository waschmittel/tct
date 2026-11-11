use super::*;

/// Tables render with box-drawing pipes in place: no extra rows, columns
/// re-aligned, header bold.
#[test]
fn snapshot_card_detail_table() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        set_description(&mut app, TABLE_DESCRIPTION);
        press(&mut app, KeyCode::Enter);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

/// The popup grows past the wrap width to fit a wide table (never past the terminal).
#[test]
fn snapshot_card_detail_wide_table_widens_popup() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        let cells = |c: &str| format!("| {} | {} | {} |", c.repeat(30), c.repeat(30), c.repeat(30));
        let table = format!("{}\n| - | - | - |\n{}", cells("h"), cells("x"));
        set_description(&mut app, &table);
        press(&mut app, KeyCode::Enter);
        insta::assert_snapshot!(render_to_string_sized(&app, 160, 30));
    });
}

/// Editing keeps the table aligned while typing, and Tab moves to the next cell.
#[test]
fn snapshot_description_editor_table() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        set_description(&mut app, TABLE_DESCRIPTION);
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char('e'));
        assert_eq!(app.mode, AppMode::Insert);
        for c in " list".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        press(&mut app, KeyCode::Tab);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

/// `Ctrl+T, t` transposes a matrix in the editor (corner stays, rows become columns).
#[test]
fn snapshot_description_editor_transpose_matrix() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        set_description(
            &mut app,
            "Plan:\n\n|  | Design | Build | Ship |\n|-|:-:|-|-|\n| Ada | owner | review | - |\n| Grace | review | owner | owner |",
        );
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char('e'));
        assert_eq!(app.mode, AppMode::Insert);
        for _ in 0..4 {
            press(&mut app, KeyCode::Down);
        }
        let ctrl_t = KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL);
        crate::input::handle_input(&mut app, ctrl_t).unwrap();
        press(&mut app, KeyCode::Char('t'));
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_card_detail_citation() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        {
            let board = app.board_mut().unwrap();
            let card_id = board.current_card_id().cloned().unwrap();
            let card = board.cards.get_mut(&card_id).unwrap();
            card.description = "Intro text.\n\n> Quoted block that is long enough to wrap across multiple lines and show hanging indentation in preview mode.".into();
        }
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_card_detail_multiline_quote() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        set_description(
            &mut app,
            "Intro.\n\n>>>\nA multiline quote with a line long enough to wrap across the popup and show that wrapped lines get no indent.\n\nSecond paragraph.\n>>>\n\nAfter.",
        );
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_card_detail_alerts() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        set_description(
            &mut app,
            "> [!note]\n> Info.\n\n>>> [!important]\nConsider:\n1. one\n>>>\n\n> [!warning]\n> Danger.",
        );
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_description_editor_wrapping() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        {
            let board = app.board_mut().unwrap();
            let card_id = board.current_card_id().cloned().unwrap();
            let card = board.cards.get_mut(&card_id).unwrap();
            card.description = [
                "This is a long introductory paragraph that wraps over multiple lines in the description editor with trailing punctuation, such as commas, and trailing periods. Here is a hyphenated-word test: multi-layered-architecture and \"quoted-text\" near boundaries.",
                "",
                "- First bullet point with a long explanation that soft-wraps neatly onto the next line while preserving list layout.",
                "  - Nested bullet point that demonstrates sub-item indentation and wrapping behavior with foo-bar-baz words.",
                "1. Numbered item that also extends well past the wrap limit to verify numbered list wrapping in the editor.",
                "> Markdown citation that also extends well past the wrap limit to verify citation indentation and wrapping in the editor.",
            ].join("\n");
        }
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        press(&mut app, KeyCode::Char('e'));
        assert_eq!(app.mode, AppMode::Insert);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

/// Footnote references and definition labels are colored; no text moves.
#[test]
fn snapshot_card_detail_footnotes() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        set_description(&mut app, FOOTNOTE_DESCRIPTION);
        press(&mut app, KeyCode::Enter);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

/// Ctrl+N adds the next reference at the cursor and lands in its new
/// definition at the very bottom; Ctrl+N there jumps back.
#[test]
fn snapshot_description_editor_footnote() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        set_description(&mut app, "Needs a source");
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char('e'));
        assert_eq!(app.mode, AppMode::Insert);
        let ctrl_n = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL);
        crate::input::handle_input(&mut app, ctrl_n).unwrap();
        crate::input::handle_paste(&mut app, "See the ADR.".to_string()).unwrap();
        insta::assert_snapshot!(render_to_string(&app));
    });
}
