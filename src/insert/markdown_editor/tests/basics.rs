use super::*;

#[test]
fn new_editor_places_cursor_at_end_of_content() {
    assert_eq!(editor_cursor(&make_editor("")), (0, 0));
    assert_eq!(editor_cursor(&make_editor("hello")), (0, 5));
    assert_eq!(editor_cursor(&make_editor("one\ntwo\nthree")), (2, 5));
    assert_eq!(editor_cursor(&make_editor("one\n")), (1, 0));
}

#[test]
fn typing_after_entering_edit_mode_appends() {
    let mut e = make_editor("abc\ndef");
    press(&mut e, KeyCode::Char('X'));
    assert_eq!(
        editor_lines(&e),
        vec!["abc".to_string(), "defX".to_string()]
    );
}

#[test]
fn help_keys_return_help_outcome() {
    let mut e = make_editor("hello");
    assert!(matches!(
        e.handle_key(KeyEvent::new(KeyCode::F(1), KeyModifiers::empty()), None),
        InsertOutcome::Help
    ));
    assert!(matches!(
        e.handle_key(
            KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL),
            None
        ),
        InsertOutcome::Help
    ));
    assert!(matches!(
        e.handle_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::SUPER), None),
        InsertOutcome::Help
    ));
    assert!(matches!(
        e.handle_key(
            KeyEvent::new(KeyCode::Char('?'), KeyModifiers::CONTROL),
            None
        ),
        InsertOutcome::Help
    ));
    // Typing bare 'h' or '?' appends text into buffer
    press(&mut e, KeyCode::Char('h'));
    press(&mut e, KeyCode::Char('?'));
    assert_eq!(editor_lines(&e), vec!["helloh?".to_string()]);
}

#[test]
fn test_copy_and_paste_with_selection() {
    let mut e = make_editor("hello world");
    e.input.textarea.move_cursor(CursorMove::Jump(0, 6));
    for _ in 0..5 {
        e.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT), None);
    }
    assert!(e.input.textarea.is_selecting());
    // Copy with Ctrl+C
    e.handle_key(
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        None,
    );
    assert_eq!(e.input.textarea.yank_text(), "world");
    assert_eq!(editor_lines(&e), vec!["hello world".to_string()]);

    // Paste at end with Ctrl+V
    e.input.textarea.move_cursor(CursorMove::End);
    e.handle_key(
        KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL),
        None,
    );
    assert_eq!(editor_lines(&e), vec!["hello worldworld".to_string()]);
}

#[test]
fn test_cut_and_paste_with_selection() {
    let mut e = make_editor("hello world");
    e.input.textarea.move_cursor(CursorMove::Jump(0, 5));
    for _ in 0..6 {
        e.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT), None);
    }
    assert!(e.input.textarea.is_selecting());
    // Cut with Ctrl+X
    e.handle_key(
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
        None,
    );
    assert_eq!(e.input.textarea.yank_text(), " world");
    assert_eq!(editor_lines(&e), vec!["hello".to_string()]);

    // Paste at end with Ctrl+V
    e.input.textarea.move_cursor(CursorMove::End);
    e.handle_key(
        KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL),
        None,
    );
    assert_eq!(editor_lines(&e), vec!["hello world".to_string()]);
}

#[test]
fn test_paste_over_selection_replaces_it() {
    let mut e = make_editor("hello world");
    crate::app::copy_to_system_clipboard("everyone");
    e.input.textarea.move_cursor(CursorMove::Jump(0, 6));
    for _ in 0..5 {
        e.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT), None);
    }
    assert!(e.input.textarea.is_selecting());
    // Paste replaces selected "world"
    e.paste_from_clipboard();
    assert_eq!(editor_lines(&e), vec!["hello everyone".to_string()]);
    assert!(!e.input.textarea.is_selecting());
}

#[test]
fn test_cut_copy_paste_keys_and_super_modifier() {
    let mut e = make_editor("abc def");
    // Cmd+C / Cmd+X without selection do not insert 'c' or 'x'
    e.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::SUPER), None);
    e.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::SUPER), None);
    assert_eq!(editor_lines(&e), vec!["abc def".to_string()]);

    // Select "def"
    e.input.textarea.move_cursor(CursorMove::Jump(0, 4));
    for _ in 0..3 {
        e.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT), None);
    }
    // Cmd+C copies
    e.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::SUPER), None);
    assert_eq!(e.input.textarea.yank_text(), "def");

    // Cmd+V pastes
    e.input.textarea.move_cursor(CursorMove::End);
    e.handle_key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::SUPER), None);
    assert_eq!(editor_lines(&e), vec!["abc defdef".to_string()]);
}

#[test]
fn test_internal_clipboard_fallback_across_instances() {
    let mut e1 = make_editor("source text");
    e1.input.textarea.move_cursor(CursorMove::Jump(0, 0));
    for _ in 0..6 {
        e1.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT), None);
    }
    e1.handle_key(
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        None,
    );

    let mut e2 = make_editor("dest: ");
    e2.paste_from_clipboard();
    assert_eq!(editor_lines(&e2), vec!["dest: source".to_string()]);
}

#[test]
fn test_highlight_and_wrap_selection() {
    let mut e = make_editor("format me please");
    e.input.textarea.move_cursor(CursorMove::Jump(0, 7));
    for _ in 0..2 {
        e.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT), None);
    }
    assert!(e.input.textarea.is_selecting());
    // Bold the selection
    e.handle_key(
        KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL),
        None,
    );
    assert_eq!(editor_lines(&e), vec!["format **me** please".to_string()]);
}

#[test]
fn bold_selection_skips_surrounding_whitespace() {
    let mut e = make_editor("one two three four");
    e.input.textarea.move_cursor(CursorMove::Jump(0, 3));
    e.input.textarea.start_selection();
    e.input.textarea.move_cursor(CursorMove::Jump(0, 14));
    ctrl(&mut e, 'b');
    assert_eq!(editor_lines(&e), vec!["one **two three** four".to_string()]);
}

#[test]
fn bold_inside_bold_removes_it() {
    let mut e = make_editor("a **bold** b");
    e.input.textarea.move_cursor(CursorMove::Jump(0, 6));
    ctrl(&mut e, 'b');
    assert_eq!(editor_lines(&e), vec!["a bold b".to_string()]);
    assert_eq!(editor_cursor(&e), (0, 4));
}

#[test]
fn emphasis_without_selection_in_table_cell_formats_cell_and_reformats() {
    let mut e = make_editor("| a | b |\n|---|---|\n| one | two |");
    e.input.textarea.move_cursor(CursorMove::Jump(2, 3));
    ctrl(&mut e, 'b');
    let lines = editor_lines(&e);
    assert!(lines[2].contains("**one**"), "{lines:?}");
    assert_eq!(lines[0].len(), lines[2].len(), "{lines:?}");
    ctrl(&mut e, 'b');
    assert!(editor_lines(&e)[2].contains("| one "));
    ctrl(&mut e, 'b');
    ctrl(&mut e, 'e');
    assert!(editor_lines(&e)[2].contains("***one***"));
}

#[test]
fn test_bracketed_paste() {
    let mut e = make_editor("title\n");
    e.handle_paste("- item 1\n- item 2");
    assert_eq!(
        editor_lines(&e),
        vec![
            "title".to_string(),
            "- item 1".to_string(),
            "- item 2".to_string()
        ]
    );
}
