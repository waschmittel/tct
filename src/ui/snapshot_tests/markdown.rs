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

/// Opens the card detail view on `text` and snapshots it.
fn snapshot_detail_of(text: &str) -> String {
    let mut out = String::new();
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        set_description(&mut app, text);
        press(&mut app, KeyCode::Enter);
        out = render_to_string_sized(&app, 100, 40);
    });
    out
}

/// A matrix has no outer borders: blank edges, cornerless header rule, bold labels.
#[test]
fn snapshot_card_detail_matrix() {
    insta::assert_snapshot!(snapshot_detail_of(
        "|  | A | B |\n|---|:-:|--:|\n| **x** | 1 | 2 |\n| y | 3 | 4 |"
    ));
}

/// Many columns and many rows (the PDF's thresholds) change nothing in the read view.
#[test]
fn snapshot_card_detail_matrix_wide_and_tall() {
    let rows: String = (1..=9)
        .map(|i| format!("\n| row {i} | {i} | {} | {} | {} |", i * 2, i * 3, i * 4))
        .collect();
    insta::assert_snapshot!(snapshot_detail_of(&format!(
        "|  | Q1 | Q2 | Q3 | Q4 |\n|---|--:|--:|--:|--:|{rows}"
    )));
}

/// Corner cases: whitespace corner, emphasis and code labels, empty row label,
/// header-only and one-column matrices, next to a normal table (keeps its borders).
#[test]
fn snapshot_card_detail_matrix_corner_cases() {
    insta::assert_snapshot!(snapshot_detail_of(
        "|   | north | south |\n|---|---|---|\n| *italic* | 1 | 2 |\n| `code` | 3 | 4 |\n|  | no label | x |\n\n|  | only header |\n|---|---|\n\n|  |\n|---|\n| lonely |\n\n| h | v |\n|---|---|\n| a | 1 |"
    ));
}

/// Column alignments (`:--`, `:-:`, `--:`); header cells are always centered.
#[test]
fn snapshot_card_detail_table_alignments() {
    insta::assert_snapshot!(snapshot_detail_of(
        "| Left | Center | Right | Default |\n|:--|:-:|--:|---|\n| a | b | c | d |\n| longer text | mid | 1234 | x |"
    ));
}

/// Many rows and columns: the read view draws the same borders regardless.
#[test]
fn snapshot_card_detail_table_wide_and_tall() {
    let rows: String = (1..=9)
        .map(|i| format!("\n| row {i} | {i} | {} | {} | {} |", i * 2, i * 3, i * 4))
        .collect();
    insta::assert_snapshot!(snapshot_detail_of(&format!(
        "| Name | Q1 | Q2 | Q3 | Q4 |\n|---|--:|--:|--:|--:|{rows}"
    )));
}

/// Cell content: emphasis, code, escaped pipes, wide (CJK/emoji) glyphs, empty cells.
#[test]
fn snapshot_card_detail_table_cell_content() {
    insta::assert_snapshot!(snapshot_detail_of(
        "| Kind | Example |\n|---|---|\n| **bold** | *italic* |\n| `code` | a \\| b |\n| 日本語 | ✓ ok |\n|  | empty left |\n| empty right |  |"
    ));
}

/// Header-only and one-column tables, and non-tables that must stay plain text.
#[test]
fn snapshot_card_detail_table_corner_cases() {
    insta::assert_snapshot!(snapshot_detail_of(
        "| only header |\n|---|\n\n| one |\n|---|\n| a |\n| b |\n\n| no | delimiter |\n| a | b |\n\n```\n| h |\n|---|\n| x |\n```"
    ));
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
