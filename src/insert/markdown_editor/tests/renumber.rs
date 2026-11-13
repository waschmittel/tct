use super::*;

#[test]
fn deleting_blank_line_between_list_items_renumbers() {
    let mut e = make_editor("1. a\n\n5. b\n7. c");
    e.input.textarea.move_cursor(CursorMove::Top);
    e.input.textarea.move_cursor(CursorMove::Down);
    e.input.textarea.move_cursor(CursorMove::Head);
    press(&mut e, KeyCode::Backspace);
    assert_eq!(
        editor_lines(&e),
        vec!["1. a".to_string(), "2. b".to_string(), "3. c".to_string()]
    );
}

#[test]
fn deleting_numbered_item_via_line_merge_renumbers() {
    let mut e = make_editor("1. a\n2. b\n3. c");
    e.input.textarea.move_cursor(CursorMove::Top);
    e.input.textarea.move_cursor(CursorMove::Down);
    e.input.textarea.move_cursor(CursorMove::Head);
    press(&mut e, KeyCode::Backspace);
    assert_eq!(
        editor_lines(&e),
        vec!["1. a2. b".to_string(), "2. c".to_string()]
    );
}

#[test]
fn typing_does_not_trigger_renumber() {
    let mut e = make_editor("1. a\n5. b");
    e.input.textarea.move_cursor(CursorMove::Top);
    e.input.textarea.move_cursor(CursorMove::End);
    press(&mut e, KeyCode::Char('x'));
    assert_eq!(
        editor_lines(&e),
        vec!["1. ax".to_string(), "5. b".to_string()]
    );
}

#[test]
fn runs_keep_their_start_number_and_count_up() {
    let mut lines = vec![
        "5. a".to_string(),
        "9. b".to_string(),
        String::new(),
        "7. c".to_string(),
        "text".to_string(),
        "3. d".to_string(),
        "   4. nested".to_string(),
        "   8. nested".to_string(),
        "0. e".to_string(),
    ];
    let mut e = make_editor(&lines.join("\n"));
    renumber_all(&mut e.input.textarea);
    lines = editor_lines(&e);
    assert_eq!(
        lines,
        [
            "5. a",
            "6. b",
            "",
            "7. c",
            "text",
            "3. d",
            "   4. nested",
            "   5. nested",
            "4. e"
        ]
    );
}

#[test]
fn renumbering_is_idempotent() {
    let mut e = make_editor("4. a\n6. b\n\n2. c");
    renumber_all(&mut e.input.textarea);
    let once = editor_lines(&e);
    renumber_all(&mut e.input.textarea);
    assert_eq!(editor_lines(&e), once);
}
