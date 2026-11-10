use super::*;

#[test]
fn enter_at_col0_of_first_list_line_inserts_blank_line_above() {
    let mut e = make_editor("- first\n- second");
    e.input.textarea.move_cursor(CursorMove::Top);
    e.input.textarea.move_cursor(CursorMove::Head);
    assert_eq!(editor_cursor(&e), (0, 0));

    press(&mut e, KeyCode::Enter);

    assert_eq!(
        editor_lines(&e),
        vec![
            "".to_string(),
            "- first".to_string(),
            "- second".to_string()
        ]
    );
    assert_eq!(editor_cursor(&e), (0, 0));
}

#[test]
fn enter_at_col0_of_inner_list_line_pushes_the_item_down() {
    let mut e = make_editor("intro\n- first\n- second");
    e.input.textarea.move_cursor(CursorMove::Top);
    e.input.textarea.move_cursor(CursorMove::Down);
    e.input.textarea.move_cursor(CursorMove::Head);
    assert_eq!(editor_cursor(&e), (1, 0));

    press(&mut e, KeyCode::Enter);

    assert_eq!(
        editor_lines(&e),
        vec![
            "intro".to_string(),
            "".to_string(),
            "- first".to_string(),
            "- second".to_string()
        ]
    );
    assert_eq!(editor_cursor(&e), (2, 0));
}

#[test]
fn enter_in_middle_of_bullet_item_splits_it() {
    let mut e = make_editor("- hello world\n- next");
    cursor_at(&mut e, 0, 7); // after "- hello"
    press(&mut e, KeyCode::Enter);
    assert_eq!(
        editor_lines(&e),
        vec![
            "- hello".to_string(),
            "- world".to_string(),
            "- next".to_string()
        ]
    );
    assert_eq!(editor_cursor(&e), (1, 2));
}

#[test]
fn enter_mid_word_splits_the_word_and_keeps_text_after_cursor() {
    let mut e = make_editor("* abcdef");
    cursor_at(&mut e, 0, 5); // "* abc|def"
    press(&mut e, KeyCode::Enter);
    assert_eq!(
        editor_lines(&e),
        vec!["* abc".to_string(), "* def".to_string()]
    );
    assert_eq!(editor_cursor(&e), (1, 2));
}

#[test]
fn enter_at_start_of_item_text_leaves_empty_item_above() {
    let mut e = make_editor("- text");
    cursor_at(&mut e, 0, 2);
    press(&mut e, KeyCode::Enter);
    assert_eq!(
        editor_lines(&e),
        vec!["- ".to_string(), "- text".to_string()]
    );
    assert_eq!(editor_cursor(&e), (1, 2));
}

#[test]
fn enter_in_nested_bullet_keeps_indent_for_the_tail() {
    let mut e = make_editor("   - one two");
    cursor_at(&mut e, 0, 8); // "   - one| two"
    press(&mut e, KeyCode::Enter);
    assert_eq!(
        editor_lines(&e),
        vec!["   - one".to_string(), "   - two".to_string()]
    );
    assert_eq!(editor_cursor(&e), (1, 5));
}

#[test]
fn enter_in_middle_of_numbered_item_splits_and_renumbers() {
    let mut e = make_editor("1. hello world\n2. next");
    cursor_at(&mut e, 0, 8); // "1. hello| world"
    press(&mut e, KeyCode::Enter);
    assert_eq!(
        editor_lines(&e),
        vec![
            "1. hello".to_string(),
            "2. world".to_string(),
            "3. next".to_string()
        ]
    );
    assert_eq!(editor_cursor(&e), (1, 3));
}

#[test]
fn enter_at_col0_of_numbered_item_pushes_it_down_without_new_item() {
    let mut e = make_editor("intro\n1. a\n2. b");
    cursor_at(&mut e, 1, 0);
    press(&mut e, KeyCode::Enter);
    assert_eq!(
        editor_lines(&e),
        vec![
            "intro".to_string(),
            "".to_string(),
            "1. a".to_string(),
            "2. b".to_string()
        ]
    );
}

#[test]
fn enter_at_end_of_list_item_continues_list() {
    let mut e = make_editor("- first");
    e.input.textarea.move_cursor(CursorMove::End);

    press(&mut e, KeyCode::Enter);

    assert_eq!(
        editor_lines(&e),
        vec!["- first".to_string(), "- ".to_string()]
    );
}

#[test]
fn enter_at_end_of_citation_continues_citation() {
    let mut e = make_editor("> first quote");
    e.input.textarea.move_cursor(CursorMove::End);
    press(&mut e, KeyCode::Enter);
    assert_eq!(
        editor_lines(&e),
        vec!["> first quote".to_string(), "> ".to_string()]
    );
    assert_eq!(editor_cursor(&e), (1, 2));
}

#[test]
fn enter_on_quote_fence_is_plain_newline() {
    let mut e = make_editor(">>>");
    e.input.textarea.move_cursor(CursorMove::End);
    press(&mut e, KeyCode::Enter);
    assert_eq!(editor_lines(&e), vec![">>>".to_string(), "".to_string()]);
}

#[test]
fn enter_on_alert_fence_is_plain_newline() {
    let mut e = make_editor(">>> [!note]");
    e.input.textarea.move_cursor(CursorMove::End);
    press(&mut e, KeyCode::Enter);
    assert_eq!(
        editor_lines(&e),
        vec![">>> [!note]".to_string(), "".to_string()]
    );
}
