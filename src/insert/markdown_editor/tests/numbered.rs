use super::*;

#[test]
fn enter_at_end_of_numbered_item_continues_with_next_number() {
    let mut e = make_editor("1. first");
    e.input.textarea.move_cursor(CursorMove::End);
    press(&mut e, KeyCode::Enter);
    assert_eq!(
        editor_lines(&e),
        vec!["1. first".to_string(), "2. ".to_string()]
    );
}

#[test]
fn enter_in_numbered_list_renumbers_following_items() {
    let mut e = make_editor("1. a\n2. b\n3. c");
    e.input.textarea.move_cursor(CursorMove::Top);
    e.input.textarea.move_cursor(CursorMove::End);
    press(&mut e, KeyCode::Enter);
    assert_eq!(
        editor_lines(&e),
        vec![
            "1. a".to_string(),
            "2. ".to_string(),
            "3. b".to_string(),
            "4. c".to_string(),
        ]
    );
    assert_eq!(editor_cursor(&e), (1, 3));
}

#[test]
fn enter_at_last_numbered_item_does_not_renumber() {
    let mut e = make_editor("1. a\n2. b");
    e.input.textarea.move_cursor(CursorMove::Bottom);
    e.input.textarea.move_cursor(CursorMove::End);
    press(&mut e, KeyCode::Enter);
    assert_eq!(
        editor_lines(&e),
        vec!["1. a".to_string(), "2. b".to_string(), "3. ".to_string()]
    );
}

#[test]
fn enter_in_parent_numbered_list_skips_nested_children() {
    let mut e = make_editor("1. parent\n   1. child\n   2. child\n2. parent2");
    e.input.textarea.move_cursor(CursorMove::Top);
    e.input.textarea.move_cursor(CursorMove::End);
    press(&mut e, KeyCode::Enter);
    assert_eq!(
        editor_lines(&e),
        vec![
            "1. parent".to_string(),
            "2. ".to_string(),
            "   1. child".to_string(),
            "   2. child".to_string(),
            "3. parent2".to_string(),
        ]
    );
}

#[test]
fn enter_in_nested_numbered_list_does_not_renumber_parents() {
    let mut e = make_editor("1. parent\n   1. child\n   2. child2\n2. parent2");
    e.input.textarea.move_cursor(CursorMove::Top);
    e.input.textarea.move_cursor(CursorMove::Down);
    e.input.textarea.move_cursor(CursorMove::End);
    press(&mut e, KeyCode::Enter);
    assert_eq!(
        editor_lines(&e),
        vec![
            "1. parent".to_string(),
            "   1. child".to_string(),
            "   2. ".to_string(),
            "   3. child2".to_string(),
            "2. parent2".to_string(),
        ]
    );
}

#[test]
fn enter_renumbers_non_canonical_numbered_list_fully() {
    let mut e = make_editor("1. a\n5. b\n7. c");
    e.input.textarea.move_cursor(CursorMove::Top);
    e.input.textarea.move_cursor(CursorMove::End);
    press(&mut e, KeyCode::Enter);
    assert_eq!(
        editor_lines(&e),
        vec![
            "1. a".to_string(),
            "2. ".to_string(),
            "3. b".to_string(),
            "4. c".to_string(),
        ]
    );
    assert_eq!(editor_cursor(&e), (1, 3));
}

#[test]
fn enter_preserves_lists_starting_at_nonzero_number() {
    let mut e = make_editor("3. a\n4. b\n4. c");
    e.input.textarea.move_cursor(CursorMove::Top);
    e.input.textarea.move_cursor(CursorMove::End);
    press(&mut e, KeyCode::Enter);
    assert_eq!(
        editor_lines(&e),
        vec![
            "3. a".to_string(),
            "4. ".to_string(),
            "5. b".to_string(),
            "6. c".to_string(),
        ]
    );
}

#[test]
fn enter_in_middle_of_list_renumbers_items_above_and_below() {
    let mut e = make_editor("1. a\n1. b\n1. c\n1. d");
    e.input.textarea.move_cursor(CursorMove::Top);
    e.input.textarea.move_cursor(CursorMove::Down);
    e.input.textarea.move_cursor(CursorMove::End);
    press(&mut e, KeyCode::Enter);
    assert_eq!(
        editor_lines(&e),
        vec![
            "1. a".to_string(),
            "2. b".to_string(),
            "3. ".to_string(),
            "4. c".to_string(),
            "5. d".to_string(),
        ]
    );
}

#[test]
fn enter_below_paragraph_renumbers_only_the_list_run() {
    let mut e = make_editor("intro text\n1. foo\n2. bar");
    e.input.textarea.move_cursor(CursorMove::Top);
    e.input.textarea.move_cursor(CursorMove::Down);
    e.input.textarea.move_cursor(CursorMove::End);
    press(&mut e, KeyCode::Enter);
    assert_eq!(
        editor_lines(&e),
        vec![
            "intro text".to_string(),
            "1. foo".to_string(),
            "2. ".to_string(),
            "3. bar".to_string(),
        ]
    );
}

#[test]
fn enter_does_not_touch_earlier_unrelated_numbered_list() {
    let mut e = make_editor("1. orphan\nunrelated\n1. foo\n2. bar");
    e.input.textarea.move_cursor(CursorMove::Top);
    e.input.textarea.move_cursor(CursorMove::Down);
    e.input.textarea.move_cursor(CursorMove::Down);
    e.input.textarea.move_cursor(CursorMove::End);
    press(&mut e, KeyCode::Enter);
    assert_eq!(
        editor_lines(&e),
        vec![
            "1. orphan".to_string(),
            "unrelated".to_string(),
            "1. foo".to_string(),
            "2. ".to_string(),
            "3. bar".to_string(),
        ]
    );
}
