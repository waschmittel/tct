use super::*;

#[test]
fn ctrl_g_chord_turns_a_quote_into_an_alert_in_one_undo_step() {
    let mut e = make_editor("> one\n> two");
    assert!(matches!(
        ctrl_out(&mut e, 'g'),
        InsertOutcome::StayWithStatus(Some(_))
    ));
    plain(&mut e, 'w');
    assert_eq!(editor_lines(&e), vec!["> [!warning]", "> one", "> two"]);
    e.input.textarea.undo();
    assert_eq!(editor_lines(&e), vec!["> one", "> two"]);
}

#[test]
fn ctrl_g_chord_in_multiline_quote_and_removal() {
    let mut e = make_editor(">>>\ntext\n>>>");
    e.input.textarea.move_cursor(CursorMove::Jump(1, 0));
    ctrl(&mut e, 'g');
    plain(&mut e, 'c');
    assert_eq!(editor_lines(&e), vec![">>> [!caution]", "text", ">>>"]);
    ctrl(&mut e, 'g');
    plain(&mut e, 'x');
    assert_eq!(editor_lines(&e), vec![">>>", "text", ">>>"]);
}

#[test]
fn ctrl_g_outside_a_quote_is_ignored_and_unknown_chord_key_is_ignored() {
    let mut e = make_editor("plain text");
    assert!(matches!(ctrl_out(&mut e, 'g'), InsertOutcome::Stay));
    plain(&mut e, 'x');
    assert_eq!(editor_lines(&e), vec!["plain textx"], "no chord armed");
    ctrl_out(&mut e, 'a');
    assert_eq!(editor_cursor(&e), (0, 0), "Ctrl+A is still line start");

    let mut e = make_editor("> q");
    ctrl(&mut e, 'g');
    plain(&mut e, 'z');
    assert_eq!(editor_lines(&e), vec!["> q"]);
}

#[test]
fn enter_on_empty_citation_clears_marker_and_exits() {
    let mut e = make_editor("> ");
    press(&mut e, KeyCode::Enter);
    assert_eq!(editor_lines(&e), vec!["".to_string(), "".to_string()]);
    assert_eq!(editor_cursor(&e), (1, 0));

    let mut e2 = make_editor(">");
    press(&mut e2, KeyCode::Enter);
    assert_eq!(editor_lines(&e2), vec!["".to_string(), "".to_string()]);
    assert_eq!(editor_cursor(&e2), (1, 0));
}

#[test]
fn enter_in_middle_of_citation_splits_it() {
    let mut e = make_editor("> hello world\n> next");
    cursor_at(&mut e, 0, 7); // after "> hello"
    press(&mut e, KeyCode::Enter);
    assert_eq!(
        editor_lines(&e),
        vec![
            "> hello".to_string(),
            "> world".to_string(),
            "> next".to_string()
        ]
    );
    assert_eq!(editor_cursor(&e), (1, 2));
}

#[test]
fn enter_in_indented_citation_keeps_indent() {
    let mut e = make_editor("  > quote text");
    e.input.textarea.move_cursor(CursorMove::End);
    press(&mut e, KeyCode::Enter);
    assert_eq!(
        editor_lines(&e),
        vec!["  > quote text".to_string(), "  > ".to_string()]
    );
    assert_eq!(editor_cursor(&e), (1, 4));
}

#[test]
fn enter_in_nested_citation_continues_nested_markers() {
    let mut e = make_editor(">> nested quote");
    e.input.textarea.move_cursor(CursorMove::End);
    press(&mut e, KeyCode::Enter);
    assert_eq!(
        editor_lines(&e),
        vec![">> nested quote".to_string(), ">> ".to_string()]
    );
    assert_eq!(editor_cursor(&e), (1, 3));

    let mut e2 = make_editor("> > spaced quote");
    e2.input.textarea.move_cursor(CursorMove::End);
    press(&mut e2, KeyCode::Enter);
    assert_eq!(
        editor_lines(&e2),
        vec!["> > spaced quote".to_string(), "> > ".to_string()]
    );
    assert_eq!(editor_cursor(&e2), (1, 4));
}

#[test]
fn enter_inside_citation_marker_pushes_the_item_down() {
    let mut e = make_editor("intro\n> quote");
    cursor_at(&mut e, 1, 0);
    press(&mut e, KeyCode::Enter);
    assert_eq!(
        editor_lines(&e),
        vec!["intro".to_string(), "".to_string(), "> quote".to_string()]
    );

    let mut e2 = make_editor("> quote");
    cursor_at(&mut e2, 0, 0);
    press(&mut e2, KeyCode::Enter);
    assert_eq!(
        editor_lines(&e2),
        vec!["".to_string(), "> quote".to_string()]
    );
}
