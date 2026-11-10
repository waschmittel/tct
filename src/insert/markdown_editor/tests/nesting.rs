use super::*;

#[test]
fn tab_nests_numbered_list_item_and_resets_to_one() {
    let mut e = make_editor("1. parent\n2. child");
    e.input.textarea.move_cursor(CursorMove::Bottom);
    e.input.textarea.move_cursor(CursorMove::End);
    press(&mut e, KeyCode::Tab);
    assert_eq!(
        editor_lines(&e),
        vec!["1. parent".to_string(), "   1. child".to_string()]
    );
}

#[test]
fn tab_nests_into_existing_nested_run() {
    let mut e = make_editor("1. parent\n   1. existing-nested\n2. orphan");
    e.input.textarea.move_cursor(CursorMove::Bottom);
    e.input.textarea.move_cursor(CursorMove::End);
    press(&mut e, KeyCode::Tab);
    assert_eq!(
        editor_lines(&e),
        vec![
            "1. parent".to_string(),
            "   1. existing-nested".to_string(),
            "   2. orphan".to_string(),
        ]
    );
}

#[test]
fn tab_nests_unordered_list_item() {
    let mut e = make_editor("- a\n- b");
    e.input.textarea.move_cursor(CursorMove::Bottom);
    e.input.textarea.move_cursor(CursorMove::End);
    press(&mut e, KeyCode::Tab);
    assert_eq!(
        editor_lines(&e),
        vec!["- a".to_string(), "   - b".to_string()]
    );
}

#[test]
fn shift_tab_unnests_nested_item_and_joins_parent_list() {
    let mut e = make_editor("1. parent\n   1. child");
    e.input.textarea.move_cursor(CursorMove::Bottom);
    e.input.textarea.move_cursor(CursorMove::End);
    press(&mut e, KeyCode::BackTab);
    assert_eq!(
        editor_lines(&e),
        vec!["1. parent".to_string(), "2. child".to_string()]
    );
}

#[test]
fn shift_tab_on_top_level_list_item_removes_marker() {
    let mut e = make_editor("1. a\n2. b");
    e.input.textarea.move_cursor(CursorMove::Bottom);
    e.input.textarea.move_cursor(CursorMove::End);
    press(&mut e, KeyCode::BackTab);
    assert_eq!(editor_lines(&e), vec!["1. a".to_string(), "b".to_string()]);
    assert_eq!(editor_cursor(&e), (1, 1));

    let mut e2 = make_editor("1. a\n2. b");
    e2.input.textarea.move_cursor(CursorMove::Top);
    e2.input.textarea.move_cursor(CursorMove::End);
    press(&mut e2, KeyCode::BackTab);
    assert_eq!(editor_lines(&e2), vec!["a".to_string(), "2. b".to_string()]);
    assert_eq!(editor_cursor(&e2), (0, 1));

    let mut e3 = make_editor("- first\n- second");
    e3.input.textarea.move_cursor(CursorMove::Bottom);
    e3.input.textarea.move_cursor(CursorMove::End);
    press(&mut e3, KeyCode::BackTab);
    assert_eq!(
        editor_lines(&e3),
        vec!["- first".to_string(), "second".to_string()]
    );
    assert_eq!(editor_cursor(&e3), (1, 6));
}

#[test]
fn shift_tab_on_top_level_citation_removes_marker() {
    let mut e = make_editor("> quote");
    e.input.textarea.move_cursor(CursorMove::End);
    press(&mut e, KeyCode::BackTab);
    assert_eq!(editor_lines(&e), vec!["quote".to_string()]);
    assert_eq!(editor_cursor(&e), (0, 5));

    let mut e2 = make_editor("> ");
    press(&mut e2, KeyCode::BackTab);
    assert_eq!(editor_lines(&e2), vec!["".to_string()]);
    assert_eq!(editor_cursor(&e2), (0, 0));
}

#[test]
fn shift_tab_on_nested_citation_decrements_quote_level() {
    let mut e = make_editor(">> quote");
    e.input.textarea.move_cursor(CursorMove::End);
    press(&mut e, KeyCode::BackTab);
    assert_eq!(editor_lines(&e), vec!["> quote".to_string()]);
    assert_eq!(editor_cursor(&e), (0, 7));

    press(&mut e, KeyCode::BackTab);
    assert_eq!(editor_lines(&e), vec!["quote".to_string()]);
    assert_eq!(editor_cursor(&e), (0, 5));

    let mut e2 = make_editor("> > spaced quote");
    e2.input.textarea.move_cursor(CursorMove::End);
    press(&mut e2, KeyCode::BackTab);
    assert_eq!(editor_lines(&e2), vec!["> spaced quote".to_string()]);
    assert_eq!(editor_cursor(&e2), (0, 14));

    press(&mut e2, KeyCode::BackTab);
    assert_eq!(editor_lines(&e2), vec!["spaced quote".to_string()]);
    assert_eq!(editor_cursor(&e2), (0, 12));
}

#[test]
fn indenting_and_unindenting_snaps_to_proper_levels() {
    // Tab from irregular spaces rounds up to next multiple of 3
    let mut e = make_editor(" - bullet");
    press(&mut e, KeyCode::Tab);
    assert_eq!(editor_lines(&e), vec!["   - bullet".to_string()]);

    let mut e = make_editor("  - bullet");
    press(&mut e, KeyCode::Tab);
    assert_eq!(editor_lines(&e), vec!["   - bullet".to_string()]);

    let mut e = make_editor("   - bullet");
    press(&mut e, KeyCode::Tab);
    assert_eq!(editor_lines(&e), vec!["      - bullet".to_string()]);

    let mut e = make_editor("    - bullet");
    press(&mut e, KeyCode::Tab);
    assert_eq!(editor_lines(&e), vec!["      - bullet".to_string()]);

    let mut e = make_editor("     - bullet");
    press(&mut e, KeyCode::Tab);
    assert_eq!(editor_lines(&e), vec!["      - bullet".to_string()]);

    // Shift+Tab from irregular spaces rounds down to previous multiple of 3
    let mut e = make_editor("     - bullet");
    press(&mut e, KeyCode::BackTab);
    assert_eq!(editor_lines(&e), vec!["   - bullet".to_string()]);

    let mut e = make_editor("    - bullet");
    press(&mut e, KeyCode::BackTab);
    assert_eq!(editor_lines(&e), vec!["   - bullet".to_string()]);

    let mut e = make_editor("   - bullet");
    press(&mut e, KeyCode::BackTab);
    assert_eq!(editor_lines(&e), vec!["- bullet".to_string()]);

    let mut e = make_editor("  - bullet");
    press(&mut e, KeyCode::BackTab);
    assert_eq!(editor_lines(&e), vec!["- bullet".to_string()]);

    let mut e = make_editor(" - bullet");
    press(&mut e, KeyCode::BackTab);
    assert_eq!(editor_lines(&e), vec!["- bullet".to_string()]);

    // Citations also snap to proper levels
    let mut e = make_editor("> quote");
    press(&mut e, KeyCode::Tab);
    assert_eq!(editor_lines(&e), vec!["   > quote".to_string()]);

    let mut e = make_editor("  > quote");
    press(&mut e, KeyCode::Tab);
    assert_eq!(editor_lines(&e), vec!["   > quote".to_string()]);

    let mut e = make_editor("    > quote");
    press(&mut e, KeyCode::Tab);
    assert_eq!(editor_lines(&e), vec!["      > quote".to_string()]);

    let mut e = make_editor("      > quote");
    press(&mut e, KeyCode::BackTab);
    assert_eq!(editor_lines(&e), vec!["   > quote".to_string()]);

    let mut e = make_editor("    > quote");
    press(&mut e, KeyCode::BackTab);
    assert_eq!(editor_lines(&e), vec!["   > quote".to_string()]);

    let mut e = make_editor("   > quote");
    press(&mut e, KeyCode::BackTab);
    assert_eq!(editor_lines(&e), vec!["> quote".to_string()]);

    let mut e = make_editor("  > quote");
    press(&mut e, KeyCode::BackTab);
    assert_eq!(editor_lines(&e), vec!["> quote".to_string()]);
}

#[test]
fn tab_numbered_item_joins_existing_deeper_bullet_list() {
    let e = nest_at("1. a\n   - x\n   - y\n2. b", 3, 4);
    assert_eq!(
        editor_lines(&e),
        vec![
            "1. a".to_string(),
            "   - x".to_string(),
            "   - y".to_string(),
            "   - b".to_string()
        ]
    );
    assert_eq!(editor_cursor(&e), (3, 6));
}

#[test]
fn tab_bullet_item_joins_existing_deeper_numbered_list_and_continues_numbers() {
    let e = nest_at("- a\n   1. x\n   2. y\n- b", 3, 3);
    assert_eq!(
        editor_lines(&e),
        vec![
            "- a".to_string(),
            "   1. x".to_string(),
            "   2. y".to_string(),
            "   3. b".to_string()
        ]
    );
    assert_eq!(editor_cursor(&e), (3, 7));
}

#[test]
fn tab_adopts_bullet_char_of_existing_deeper_list() {
    let e = nest_at("- a\n   * x\n- b", 2, 3);
    assert_eq!(
        editor_lines(&e),
        vec![
            "- a".to_string(),
            "   * x".to_string(),
            "   * b".to_string()
        ]
    );
}

#[test]
fn tab_cursor_follows_text_when_marker_is_rewritten() {
    let e = nest_at("1. a\n   - x\n2. bee", 2, 6);
    assert_eq!(editor_lines(&e)[2], "   - bee");
    assert_eq!(editor_cursor(&e), (2, 8));
}

#[test]
fn tab_does_not_adopt_scheme_across_a_blank_line_or_without_deeper_list() {
    let e = nest_at("1. a\n   - x\n\n2. b", 3, 4);
    assert_eq!(editor_lines(&e)[3], "   1. b");
    let e = nest_at("- a\n1. b", 1, 4);
    assert_eq!(
        editor_lines(&e),
        vec!["- a".to_string(), "   1. b".to_string()]
    );
}

#[test]
fn tab_then_shift_tab_restores_the_item() {
    let mut e = nest_at("- a\n   1. x\n- b", 2, 3);
    assert_eq!(editor_lines(&e)[2], "   2. b");
    press(&mut e, KeyCode::BackTab);
    assert_eq!(
        editor_lines(&e),
        vec!["- a".to_string(), "   1. x".to_string(), "- b".to_string()]
    );
    assert_eq!(editor_cursor(&e), (2, 3));
}

#[test]
fn shift_tab_numbered_child_of_bullet_becomes_bullet() {
    let e = unnest_at("- parent\n   1. child", 1, 12);
    assert_eq!(
        editor_lines(&e),
        vec!["- parent".to_string(), "- child".to_string()]
    );
    assert_eq!(editor_cursor(&e), (1, 7));
}

#[test]
fn shift_tab_bullet_child_of_numbered_continues_numbering() {
    let e = unnest_at("1. a\n2. b\n   - c", 2, 8);
    assert_eq!(
        editor_lines(&e),
        vec!["1. a".to_string(), "2. b".to_string(), "3. c".to_string()]
    );
    assert_eq!(editor_cursor(&e), (2, 4));
}

#[test]
fn shift_tab_numbered_child_continues_parent_numbers_and_renumbers_below() {
    let e = unnest_at("1. a\n   1. x\n   2. y\n2. b", 2, 7);
    assert_eq!(
        editor_lines(&e),
        vec![
            "1. a".to_string(),
            "   1. x".to_string(),
            "2. y".to_string(),
            "3. b".to_string()
        ]
    );
}

#[test]
fn shift_tab_adopts_parent_bullet_char() {
    let e = unnest_at("* a\n   - b", 1, 5);
    assert_eq!(editor_lines(&e), vec!["* a".to_string(), "* b".to_string()]);
}

#[test]
fn shift_tab_skips_deeper_lines_to_find_parent_scheme() {
    let e = unnest_at("- a\n   1. x\n      - deep\n   2. y", 3, 7);
    assert_eq!(
        editor_lines(&e),
        vec![
            "- a".to_string(),
            "   1. x".to_string(),
            "      - deep".to_string(),
            "- y".to_string()
        ]
    );
}

#[test]
fn shift_tab_cursor_before_marker_only_loses_indent() {
    let e = unnest_at("- a\n   1. x", 1, 3);
    assert_eq!(editor_lines(&e), vec!["- a".to_string(), "- x".to_string()]);
    assert_eq!(editor_cursor(&e), (1, 0));
}

#[test]
fn shift_tab_without_a_parent_item_keeps_scheme() {
    let e = unnest_at("intro\n   - a", 1, 5);
    assert_eq!(
        editor_lines(&e),
        vec!["intro".to_string(), "- a".to_string()]
    );
    let e = unnest_at("\n   1. a", 1, 6);
    assert_eq!(editor_lines(&e), vec!["".to_string(), "1. a".to_string()]);
}
