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

// ── Tab / Shift+Tab with a selection ─────────────────────────────────

fn lines_of(e: &MarkdownEditor) -> Vec<&str> {
    e.input
        .textarea
        .lines()
        .iter()
        .map(String::as_str)
        .collect()
}

fn selected(e: &MarkdownEditor) -> Option<((usize, usize), (usize, usize))> {
    e.input.textarea.selection_range()
}

#[test]
fn tab_with_selection_nests_all_selected_list_items() {
    let mut e = make_editor("- a\n- b\n- c\n- d");
    select(&mut e, (1, 2), (2, 3));
    press(&mut e, KeyCode::Tab);
    assert_eq!(lines_of(&e), ["- a", "   - b", "   - c", "- d"]);
}

#[test]
fn shift_tab_with_selection_unnests_all_selected_items() {
    let mut e = make_editor("- a\n   - b\n   - c\n- d");
    select(&mut e, (1, 5), (2, 5));
    press(&mut e, KeyCode::BackTab);
    assert_eq!(lines_of(&e), ["- a", "- b", "- c", "- d"]);
}

#[test]
fn shift_tab_on_selected_top_level_items_removes_markers_and_keeps_run_start() {
    let mut e = make_editor("1. a\n2. b\n3. c\n4. d");
    select(&mut e, (0, 0), (1, 4));
    press(&mut e, KeyCode::BackTab);
    assert_eq!(lines_of(&e), ["a", "b", "3. c", "4. d"]);
}

#[test]
fn tab_with_selection_renumbers_numbered_lists() {
    let mut e = make_editor("1. a\n2. b\n3. c\n4. d");
    select(&mut e, (1, 0), (2, 4));
    press(&mut e, KeyCode::Tab);
    assert_eq!(lines_of(&e), ["1. a", "   1. b", "   2. c", "2. d"]);
}

#[test]
fn tab_with_selection_never_deletes_the_selected_text() {
    // Previously the selection was replaced by spaces.
    let mut e = make_editor("hello world");
    select(&mut e, (0, 0), (0, 5));
    press(&mut e, KeyCode::Tab);
    assert_eq!(lines_of(&e), ["   hello world"]);
    // Selection grew by the indent and still covers "hello".
    assert_eq!(selected(&e), Some(((0, 0), (0, 8))));
}

#[test]
fn tab_with_selection_indents_plain_lines_and_skips_blank_ones() {
    let mut e = make_editor("one\n\ntwo\n  \nthree");
    select(&mut e, (0, 0), (4, 5));
    press(&mut e, KeyCode::Tab);
    assert_eq!(lines_of(&e), ["   one", "", "   two", "  ", "   three"]);
    assert_eq!(selected(&e), Some(((0, 0), (4, 8))));
}

#[test]
fn shift_tab_with_selection_removes_up_to_three_spaces_or_a_tab() {
    let mut e = make_editor("      six\n  two\none\n\ttab\n    four");
    select(&mut e, (0, 0), (4, 8));
    press(&mut e, KeyCode::BackTab);
    assert_eq!(lines_of(&e), ["   six", "two", "one", "tab", " four"]);
}

#[test]
fn mixed_plain_and_list_lines_shift_together() {
    let mut e = make_editor("- a\n  more\n- b");
    select(&mut e, (0, 0), (2, 3));
    press(&mut e, KeyCode::Tab);
    assert_eq!(lines_of(&e), ["   - a", "     more", "   - b"]);
}

#[test]
fn cursor_row_counts_even_at_column_zero() {
    let mut e = make_editor("- a\n- b\n- c");
    select(&mut e, (0, 0), (2, 0));
    press(&mut e, KeyCode::Tab);
    assert_eq!(lines_of(&e), ["   - a", "   - b", "   - c"]);
    press(&mut e, KeyCode::BackTab);
    assert_eq!(lines_of(&e), ["- a", "- b", "- c"]);
}

#[test]
fn anchor_at_column_zero_of_a_later_row_is_excluded() {
    let mut e = make_editor("- a\n- b\n- c");
    select(&mut e, (2, 0), (0, 2));
    press(&mut e, KeyCode::Tab);
    assert_eq!(lines_of(&e), ["   - a", "   - b", "- c"]);
}

#[test]
fn selection_within_one_row_ending_at_column_zero_counts() {
    let mut e = make_editor("- a\n- b");
    select(&mut e, (1, 3), (1, 0));
    press(&mut e, KeyCode::Tab);
    assert_eq!(lines_of(&e), ["- a", "   - b"]);
}

#[test]
fn backwards_selection_keeps_its_direction() {
    let mut e = make_editor("- a\n- b\n- c");
    select(&mut e, (2, 3), (0, 2));
    press(&mut e, KeyCode::Tab);
    assert_eq!(lines_of(&e), ["   - a", "   - b", "   - c"]);
    assert_eq!(selected(&e), Some(((0, 5), (2, 6))));
    assert_eq!(editor_cursor(&e), (0, 5));
}

#[test]
fn selection_survives_and_repeated_tab_goes_deeper() {
    let mut e = make_editor("- a\n- b\n- c");
    select(&mut e, (0, 0), (2, 3));
    press(&mut e, KeyCode::Tab);
    press(&mut e, KeyCode::Tab);
    assert_eq!(lines_of(&e), ["      - a", "      - b", "      - c"]);
    press(&mut e, KeyCode::BackTab);
    press(&mut e, KeyCode::BackTab);
    assert_eq!(lines_of(&e), ["- a", "- b", "- c"]);
    assert!(selected(&e).is_some());
}

#[test]
fn selection_endpoints_follow_text_when_numbers_grow() {
    let mut e = make_editor("9. a\n10. b\n11. c");
    select(&mut e, (1, 4), (2, 7));
    press(&mut e, KeyCode::BackTab);
    // Top-level markers are removed; the selection still ends at the end of "c".
    assert_eq!(lines_of(&e), ["9. a", "b", "c"]);
    assert_eq!(selected(&e), Some(((1, 0), (2, 1))));
}

#[test]
fn citations_in_a_selection_nest_and_unnest() {
    let mut e = make_editor("> a\n> b");
    select(&mut e, (0, 0), (1, 3));
    press(&mut e, KeyCode::Tab);
    assert_eq!(lines_of(&e), ["   > a", "   > b"]);
    press(&mut e, KeyCode::BackTab);
    assert_eq!(lines_of(&e), ["> a", "> b"]);
    press(&mut e, KeyCode::BackTab);
    assert_eq!(lines_of(&e), ["a", "b"]);
}

#[test]
fn table_rows_in_a_selection_are_left_alone() {
    let mut e = make_editor("text\n| a | b |\n|---|---|\n| 1 | 2 |\nmore");
    select(&mut e, (0, 0), (4, 4));
    press(&mut e, KeyCode::Tab);
    assert_eq!(
        lines_of(&e),
        ["   text", "| a | b |", "|---|---|", "| 1 | 2 |", "   more"]
    );
}

#[test]
fn selection_inside_a_table_still_walks_cells() {
    let mut e = make_editor("| a | b |\n|---|---|\n| 1 | 2 |");
    select(&mut e, (0, 2), (0, 3));
    press(&mut e, KeyCode::Tab);
    assert!(lines_of(&e)[0].starts_with('|'));
    assert_eq!(lines_of(&e).len(), 3);
}

#[test]
fn tab_with_selection_is_one_undo_step() {
    let mut e = make_editor("- a\n- b\n- c");
    select(&mut e, (0, 0), (2, 3));
    press(&mut e, KeyCode::Tab);
    ctrl(&mut e, 'z');
    assert_eq!(lines_of(&e), ["- a", "- b", "- c"]);
}

#[test]
fn shift_tab_with_selection_is_one_undo_step() {
    let mut e = make_editor("1. a\n2. b\n3. c");
    select(&mut e, (0, 0), (2, 4));
    press(&mut e, KeyCode::BackTab);
    ctrl(&mut e, 'z');
    assert_eq!(lines_of(&e), ["1. a", "2. b", "3. c"]);
}

#[test]
fn empty_selection_behaves_like_no_selection() {
    let mut e = make_editor("- a\n- b");
    cursor_at(&mut e, 1, 3);
    e.input.textarea.start_selection();
    press(&mut e, KeyCode::Tab);
    assert_eq!(lines_of(&e), ["- a", "   - b"]);
}

#[test]
fn shift_tab_with_selection_on_unindented_plain_text_is_a_noop() {
    let mut e = make_editor("a\nb");
    select(&mut e, (0, 0), (1, 1));
    press(&mut e, KeyCode::BackTab);
    assert_eq!(lines_of(&e), ["a", "b"]);
    assert_eq!(selected(&e), Some(((0, 0), (1, 1))));
}

#[test]
fn whole_document_selection_via_select_all() {
    let mut e = make_editor("- a\n- b");
    e.input.textarea.select_all();
    press(&mut e, KeyCode::Tab);
    assert_eq!(lines_of(&e), ["   - a", "   - b"]);
}
