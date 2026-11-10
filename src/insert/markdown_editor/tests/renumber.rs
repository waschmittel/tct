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
