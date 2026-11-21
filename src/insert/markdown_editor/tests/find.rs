use super::*;

fn type_str(e: &mut MarkdownEditor, s: &str) {
    for c in s.chars() {
        plain(e, c);
    }
}

fn alt(e: &mut MarkdownEditor, c: char) -> InsertOutcome {
    e.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::ALT), None)
}

fn shift(e: &mut MarkdownEditor, code: KeyCode) -> InsertOutcome {
    e.handle_key(KeyEvent::new(code, KeyModifiers::SHIFT), None)
}

#[test]
fn ctrl_f_defaults_to_case_insensitive_regex() {
    let mut e = make_editor("Token one\ntoken two\nT0KEN three");
    cursor_at(&mut e, 0, 0);
    ctrl(&mut e, 'f');

    let f = e.find_state().expect("find bar open");
    assert!(!f.replace_mode);
    assert!(f.use_regex, "regex should be enabled by default");
    assert!(!f.case_sensitive, "case-insensitive by default");
    assert_eq!(f.focus, FindFocus::Query);

    type_str(&mut e, "t.ken");
    let f = e.find_state().unwrap();
    assert_eq!(f.matches.len(), 3);
    assert_eq!(f.current, Some(0));
    assert_eq!(f.status_label().as_deref(), Some("1/3"));
    assert_eq!(editor_cursor(&e), (0, 0));
    assert!(e.input.textarea.search_pattern().is_some());
}

#[test]
fn navigation_cycles_forward_and_backward_with_wrap() {
    let mut e = make_editor("foo bar\nbaz foo\nfoo");
    cursor_at(&mut e, 0, 0);
    ctrl(&mut e, 'f');
    type_str(&mut e, "foo");

    assert_eq!(editor_cursor(&e), (0, 0));
    assert_eq!(
        e.find_state().unwrap().status_label().as_deref(),
        Some("1/3")
    );

    press(&mut e, KeyCode::Enter);
    assert_eq!(editor_cursor(&e), (1, 4));
    assert_eq!(
        e.find_state().unwrap().status_label().as_deref(),
        Some("2/3")
    );

    press(&mut e, KeyCode::Down);
    assert_eq!(editor_cursor(&e), (2, 0));
    assert_eq!(
        e.find_state().unwrap().status_label().as_deref(),
        Some("3/3")
    );

    // Wraps to first match
    press(&mut e, KeyCode::F(3));
    assert_eq!(editor_cursor(&e), (0, 0));
    assert_eq!(
        e.find_state().unwrap().status_label().as_deref(),
        Some("1/3")
    );

    // Backward wraps to last match
    press(&mut e, KeyCode::Up);
    assert_eq!(editor_cursor(&e), (2, 0));
    assert_eq!(
        e.find_state().unwrap().status_label().as_deref(),
        Some("3/3")
    );

    shift(&mut e, KeyCode::F(3));
    assert_eq!(editor_cursor(&e), (1, 4));
    assert_eq!(
        e.find_state().unwrap().status_label().as_deref(),
        Some("2/3")
    );

    shift(&mut e, KeyCode::Enter);
    assert_eq!(editor_cursor(&e), (0, 0));
}

#[test]
fn tab_cycles_find_chips_and_toggles_case_and_regex() {
    let mut e = make_editor("Check [x] and X");
    cursor_at(&mut e, 0, 0);
    ctrl(&mut e, 'f');
    type_str(&mut e, "[x]");

    // In regex mode, `[x]` matches both `x` and `X` (2 matches).
    assert_eq!(e.find_state().unwrap().matches.len(), 2);

    // Tab: Query -> CaseSensitive -> Regex
    press(&mut e, KeyCode::Tab);
    assert_eq!(e.find_state().unwrap().focus, FindFocus::CaseSensitive);
    press(&mut e, KeyCode::Char(' '));
    assert!(e.find_state().unwrap().case_sensitive);
    // Case-sensitive `[x]` matches only lowercase `x` (1 match).
    assert_eq!(e.find_state().unwrap().matches.len(), 1);
    assert_eq!(editor_cursor(&e), (0, 7));

    press(&mut e, KeyCode::Tab);
    assert_eq!(e.find_state().unwrap().focus, FindFocus::Regex);
    press(&mut e, KeyCode::Enter);
    assert!(!e.find_state().unwrap().use_regex);
    // Literal `[x]` matches the bracketed token at col 6.
    assert_eq!(e.find_state().unwrap().matches.len(), 1);
    assert_eq!(editor_cursor(&e), (0, 6));

    // Tab wraps back to Query; BackTab goes to Regex.
    press(&mut e, KeyCode::Tab);
    assert_eq!(e.find_state().unwrap().focus, FindFocus::Query);
    press(&mut e, KeyCode::BackTab);
    assert_eq!(e.find_state().unwrap().focus, FindFocus::Regex);

    // Shortcut toggles also work.
    alt(&mut e, 'c');
    assert!(!e.find_state().unwrap().case_sensitive);
    alt(&mut e, 'x');
    assert!(e.find_state().unwrap().use_regex);
}

#[test]
fn invalid_regex_is_handled_gracefully_and_literal_mode_recovers() {
    let mut e = make_editor("call(arg)");
    cursor_at(&mut e, 0, 0);
    ctrl(&mut e, 'f');
    type_str(&mut e, "(");

    let f = e.find_state().unwrap();
    assert!(f.regex_error);
    assert!(f.matches.is_empty());
    assert_eq!(f.status_label().as_deref(), Some("invalid"));
    assert!(e.input.textarea.search_pattern().is_none());

    // Turn regex off via F5 -> literal "(" matches.
    press(&mut e, KeyCode::F(5));
    let f = e.find_state().unwrap();
    assert!(!f.regex_error);
    assert_eq!(f.matches.len(), 1);
    assert_eq!(editor_cursor(&e), (0, 4));
}

#[test]
fn ctrl_r_tab_cycle_covers_all_replace_chips() {
    let mut e = make_editor("alpha beta alpha");
    cursor_at(&mut e, 0, 0);
    ctrl(&mut e, 'r');

    let expected = [
        FindFocus::Query,
        FindFocus::Replace,
        FindFocus::ReplaceOne,
        FindFocus::ReplaceAll,
        FindFocus::CaseSensitive,
        FindFocus::Regex,
        FindFocus::Query,
    ];
    for &focus in &expected {
        assert_eq!(e.find_state().unwrap().focus, focus);
        press(&mut e, KeyCode::Tab);
    }
}

#[test]
fn replace_one_advances_past_self_matching_replacement_and_undoes_in_one_step() {
    let mut e = make_editor("a b a");
    cursor_at(&mut e, 0, 0);
    ctrl(&mut e, 'r');
    type_str(&mut e, "a");
    press(&mut e, KeyCode::Tab);
    type_str(&mut e, "aa");

    // Enter in Replace replaces the first "a" and advances to the second.
    press(&mut e, KeyCode::Enter);
    assert_eq!(editor_lines(&e), vec!["aa b a"]);
    assert_eq!(editor_cursor(&e), (0, 5));

    // Activate [Replace] chip via Tab + Space.
    press(&mut e, KeyCode::Tab);
    assert_eq!(e.find_state().unwrap().focus, FindFocus::ReplaceOne);
    press(&mut e, KeyCode::Char(' '));
    assert_eq!(editor_lines(&e), vec!["aa b aa"]);

    // One Ctrl+Z undoes the second replacement only.
    ctrl(&mut e, 'z');
    assert_eq!(editor_lines(&e), vec!["aa b a"]);
    ctrl(&mut e, 'z');
    assert_eq!(editor_lines(&e), vec!["a b a"]);
}

#[test]
fn replace_all_supports_regex_captures_utf8_and_single_undo() {
    let mut e = make_editor("ä=1 ö=2\nü=3 🎉=4");
    cursor_at(&mut e, 0, 0);
    ctrl(&mut e, 'r');
    type_str(&mut e, r"(\S+)=(\d+)");
    press(&mut e, KeyCode::Tab);
    type_str(&mut e, "$1:$2");

    // Tab to [All] chip and activate with Enter.
    press(&mut e, KeyCode::Tab);
    press(&mut e, KeyCode::Tab);
    assert_eq!(e.find_state().unwrap().focus, FindFocus::ReplaceAll);
    let out = e.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()), None);
    assert!(matches!(
        out,
        InsertOutcome::StayWithStatus(Some(ref s)) if s == "Replaced 4 occurrences"
    ));
    assert_eq!(editor_lines(&e), vec!["ä:1 ö:2", "ü:3 🎉:4"]);

    // Single Ctrl+Z reverts the entire Replace All.
    ctrl(&mut e, 'z');
    assert_eq!(editor_lines(&e), vec!["ä=1 ö=2", "ü=3 🎉=4"]);
}

#[test]
fn replace_all_handles_zero_length_anchor() {
    let mut e = make_editor("one\ntwo");
    cursor_at(&mut e, 0, 0);
    ctrl(&mut e, 'r');
    type_str(&mut e, "^");
    press(&mut e, KeyCode::Tab);
    type_str(&mut e, "> ");

    let out = ctrl_out(&mut e, 'g');
    assert!(matches!(
        out,
        InsertOutcome::StayWithStatus(Some(ref s)) if s == "Replaced 2 occurrences"
    ));
    assert_eq!(editor_lines(&e), vec!["> one", "> two"]);
}

#[test]
fn replace_inside_table_reflows_unless_paste_mode() {
    let table = "| a | b |\n| - | - |\n| x | y |";
    let mut e = make_editor(table);
    cursor_at(&mut e, 0, 0);
    ctrl(&mut e, 'r');
    type_str(&mut e, "x");
    press(&mut e, KeyCode::Tab);
    type_str(&mut e, "longer");
    press(&mut e, KeyCode::Enter);
    assert_eq!(
        editor_lines(&e),
        vec!["|   a    |  b  |", "| ------ | --- |", "| longer | y   |"]
    );

    // In paste mode, table reflow is skipped.
    let mut e2 = make_editor(table);
    paste_mode_on(&mut e2);
    cursor_at(&mut e2, 0, 0);
    ctrl(&mut e2, 'r');
    type_str(&mut e2, "x");
    press(&mut e2, KeyCode::Tab);
    type_str(&mut e2, "longer");
    press(&mut e2, KeyCode::Enter);
    assert_eq!(
        editor_lines(&e2),
        vec!["| a | b |", "| - | - |", "| longer | y |"]
    );
}

#[test]
fn prefill_from_single_line_selection_escapes_in_regex_mode() {
    let mut e = make_editor("todo [x] and [x]");
    select(&mut e, (0, 5), (0, 8));
    ctrl(&mut e, 'f');

    let f = e.find_state().unwrap();
    assert_eq!(f.query.buffer, r"\[x\]");
    assert_eq!(f.matches.len(), 2);
    assert_eq!(editor_cursor(&e), (0, 5));
}

#[test]
fn esc_closes_bar_without_triggering_discard_prompt_and_f3_repeats() {
    let mut e = make_editor("alpha beta alpha");
    cursor_at(&mut e, 0, 0);
    // Modify document so Esc from the editor would open ConfirmCancelEdit.
    plain(&mut e, '!');
    assert!(e.input.is_modified());

    ctrl(&mut e, 'f');
    type_str(&mut e, "alpha");
    press(&mut e, KeyCode::Enter); // second match
    assert_eq!(editor_cursor(&e), (0, 12));

    // Esc closes Find bar, clears pattern, keeps cursor, stays in editor.
    let out = e.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()), None);
    assert!(matches!(out, InsertOutcome::Stay));
    assert!(e.find_state().is_none());
    assert!(e.input.textarea.search_pattern().is_none());
    assert_eq!(editor_cursor(&e), (0, 12));

    // F3 repeats last search forward (wraps to col 1).
    press(&mut e, KeyCode::F(3));
    assert_eq!(editor_cursor(&e), (0, 1));
    // Shift+F3 repeats backward (wraps to col 12).
    shift(&mut e, KeyCode::F(3));
    assert_eq!(editor_cursor(&e), (0, 12));

    // Second Esc now opens ConfirmCancelEdit as usual.
    let out = e.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()), None);
    assert!(matches!(out, InsertOutcome::OpenDialogOverInsert(_)));
}

#[test]
fn bracketed_paste_routes_into_focused_find_input() {
    let mut e = make_editor("hello world");
    cursor_at(&mut e, 0, 0);
    ctrl(&mut e, 'f');
    e.handle_paste("world\n");

    assert_eq!(editor_lines(&e), vec!["hello world"]);
    assert_eq!(e.find_state().unwrap().query.buffer, "world ");
}

#[test]
fn ctrl_s_and_ctrl_q_from_find_bar_close_bar_and_save() {
    let mut e = make_editor("hello world");
    cursor_at(&mut e, 0, 0);
    ctrl(&mut e, 'r');
    type_str(&mut e, "world");
    press(&mut e, KeyCode::Tab);
    type_str(&mut e, "tct");
    press(&mut e, KeyCode::Enter);

    let out = ctrl_out(&mut e, 's');
    assert!(matches!(out, InsertOutcome::Save(_)));
    assert!(e.find_state().is_none());
    assert!(e.input.textarea.search_pattern().is_none());

    ctrl(&mut e, 'r');
    type_str(&mut e, "!");
    press(&mut e, KeyCode::Tab);
    press(&mut e, KeyCode::Tab);
    // Now test Ctrl+Q after another change
    ctrl(&mut e, 'f');
    let out = ctrl_out(&mut e, 'q');
    // No change since Ctrl+S -> Cancel
    assert!(matches!(out, InsertOutcome::Cancel));
    assert!(e.find_state().is_none());
}
