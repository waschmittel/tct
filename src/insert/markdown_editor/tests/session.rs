use super::*;

#[test]
fn ctrl_s_returns_save_and_resets_is_modified() {
    let mut e = make_editor("initial");
    press(&mut e, KeyCode::Char('!'));
    assert!(e.input.is_modified());

    let out = e.handle_key(
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
        None,
    );
    match out {
        InsertOutcome::Save(Command::EditCardDescription { card_id, body, .. }) => {
            assert_eq!(card_id, "card1");
            assert_eq!(body, "initial!");
        }
        _ => panic!("expected InsertOutcome::Save"),
    }
    // After Ctrl+S, is_modified is reset
    assert!(!e.input.is_modified());

    // Subsequent Esc should now cancel directly, not prompt
    let esc_out = e.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()), None);
    assert!(matches!(esc_out, InsertOutcome::Cancel));
}

#[test]
fn ctrl_q_returns_confirm() {
    let mut e = make_editor("initial");
    press(&mut e, KeyCode::Char('!'));

    let out = e.handle_key(
        KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL),
        None,
    );
    match out {
        InsertOutcome::Confirm(Command::EditCardDescription { card_id, body, .. }) => {
            assert_eq!(card_id, "card1");
            assert_eq!(body, "initial!");
        }
        _ => panic!("expected InsertOutcome::Confirm"),
    }
}

#[test]
fn typing_bare_s_and_q_inserts_characters() {
    let mut e = make_editor("hello ");
    press(&mut e, KeyCode::Char('s'));
    press(&mut e, KeyCode::Char('q'));
    assert_eq!(editor_lines(&e), vec!["hello sq".to_string()]);
}

#[test]
fn paste_mode_inserts_plain_text_without_list_or_table_magic() {
    let mut e = make_editor("");
    paste_mode_on(&mut e);
    e.handle_paste("1. a\n5. b\n|a|b|\n|-|-|\n|long cell|c|");
    assert_eq!(
        editor_lines(&e),
        ["1. a", "5. b", "|a|b|", "|-|-|", "|long cell|c|"]
    );
    // Enter after a list item does not continue the list.
    press(&mut e, KeyCode::Enter);
    assert_eq!(editor_lines(&e).len(), 6);
    assert_eq!(editor_lines(&e)[5], "");
}

#[test]
fn leaving_paste_mode_normalizes_in_one_undo_step() {
    let mut e = make_editor("");
    paste_mode_on(&mut e);
    e.handle_paste(
        "1. a\n5. b\n  - x\n    - y\n\t- z\n|a|b|\n|-|-|\n|long cell|c|\n```\n  - keep\n```",
    );
    ctrl(&mut e, 'p');
    assert!(!e.paste_mode());
    let lines = editor_lines(&e);
    assert_eq!(&lines[..3], ["1. a", "2. b", "   - x"]);
    assert_eq!(&lines[3..5], ["      - y", "      - z"]);
    assert!(
        lines[5].contains("long cell") || lines[7].contains("long cell"),
        "{lines:?}"
    );
    assert!(lines[5].len() == lines[7].len(), "{lines:?}");
    assert_eq!(&lines[8..], ["```", "  - keep", "```"]);
    e.input.textarea.undo();
    assert_eq!(editor_lines(&e)[1], "5. b");
}

#[test]
fn saving_leaves_paste_mode_and_saves_normalized_text() {
    let mut e = make_editor("");
    paste_mode_on(&mut e);
    e.handle_paste("1. a\n5. b");
    match ctrl_out(&mut e, 's') {
        InsertOutcome::Save(Command::EditCardDescription { body, .. }) => {
            assert_eq!(body, "1. a\n2. b")
        }
        _ => panic!("expected Save"),
    }
    assert!(!e.paste_mode());
}

#[test]
fn paste_mode_keys_are_plain() {
    let mut e = make_editor("- a");
    paste_mode_on(&mut e);
    press(&mut e, KeyCode::Enter);
    assert_eq!(editor_lines(&e), ["- a", ""]);
}

fn typed(e: &mut MarkdownEditor, text: &str) {
    for c in text.chars() {
        press(e, KeyCode::Char(c));
    }
}

#[test]
fn a_typing_burst_undoes_by_word_and_redoes_whole() {
    let mut e = make_editor("");
    typed(&mut e, "hello world");
    ctrl(&mut e, 'z');
    assert_eq!(editor_lines(&e), ["hello "]);
    ctrl(&mut e, 'z');
    assert_eq!(editor_lines(&e), [""]);
    ctrl(&mut e, 'y');
    ctrl(&mut e, 'y');
    assert_eq!(editor_lines(&e), ["hello world"]);
}

/// The only test that sleeps: the editor reads the real clock.
#[test]
fn a_pause_splits_typing_steps() {
    let mut e = make_editor("");
    typed(&mut e, "one");
    std::thread::sleep(std::time::Duration::from_millis(600));
    typed(&mut e, "two");
    ctrl(&mut e, 'z');
    assert_eq!(editor_lines(&e), ["one"]);
}

#[test]
fn a_structural_edit_splits_typing_steps() {
    let mut e = make_editor("");
    typed(&mut e, "a");
    press(&mut e, KeyCode::Enter);
    typed(&mut e, "b");
    ctrl(&mut e, 'z');
    assert_eq!(editor_lines(&e), ["a", ""]);
}

#[test]
fn undo_history_reaches_past_fifty_edits() {
    let mut e = make_editor("");
    for _ in 0..120 {
        typed(&mut e, "x");
        press(&mut e, KeyCode::Enter);
    }
    for _ in 0..240 {
        ctrl(&mut e, 'z');
    }
    assert_eq!(editor_lines(&e), [""]);
}

// ── Remembered cursor ────────────────────────────────────────────────

fn doc(n: usize) -> String {
    (0..n)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn restore_puts_the_cursor_back() {
    let mut e = make_editor(&doc(50));
    e.restore((20, 3));
    assert_eq!(editor_cursor(&e), (20, 3));
}

#[test]
fn restore_clamps_when_the_text_got_shorter() {
    let mut e = make_editor("short\ntext");
    e.restore((40, 6));
    assert_eq!(editor_cursor(&e), (1, 4));
}

#[test]
fn restore_survives_absurd_values_from_a_hostile_file() {
    let mut e = make_editor("short\ntext");
    e.restore((usize::MAX, usize::MAX));
    assert_eq!(editor_cursor(&e), (1, 4));
    e.restore((65_536, 0)); // would wrap to row 0 with a bare `as u16`
    assert_eq!(editor_cursor(&e), (1, 0));
}

#[test]
fn saving_reports_the_cursor() {
    let mut e = make_editor("one\ntwo");
    plain(&mut e, '!');
    cursor_at(&mut e, 0, 2);
    let save = e.handle_key(
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
        None,
    );
    assert!(matches!(
        save,
        InsertOutcome::Save(Command::EditCardDescription {
            cursor: Some((0, 2)),
            ..
        })
    ));
    plain(&mut e, '?');
    let quit = e.handle_key(
        KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL),
        None,
    );
    assert!(matches!(
        quit,
        InsertOutcome::Confirm(Command::EditCardDescription {
            cursor: Some((0, 3)),
            ..
        })
    ));
}

#[test]
fn nothing_is_saved_when_only_the_cursor_moved() {
    let mut e = make_editor("one\ntwo");
    cursor_at(&mut e, 0, 2);
    assert!(matches!(ctrl_out(&mut e, 's'), InsertOutcome::Stay));
    assert!(matches!(ctrl_out(&mut e, 'q'), InsertOutcome::Cancel));
}

#[test]
fn typing_and_undoing_back_to_the_original_is_not_a_change() {
    let mut e = make_editor("one");
    plain(&mut e, 'x');
    ctrl(&mut e, 'z');
    assert!(matches!(ctrl_out(&mut e, 's'), InsertOutcome::Stay));
}

#[test]
fn second_save_without_edits_is_a_noop() {
    let mut e = make_editor("one");
    plain(&mut e, 'x');
    assert!(matches!(ctrl_out(&mut e, 's'), InsertOutcome::Save(_)));
    assert!(matches!(ctrl_out(&mut e, 's'), InsertOutcome::Stay));
    assert!(matches!(ctrl_out(&mut e, 'q'), InsertOutcome::Cancel));
}

#[test]
fn paste_mode_normalizing_to_the_original_is_not_a_change() {
    let mut e = make_editor("1. a\n2. b");
    paste_mode_on(&mut e);
    assert!(matches!(ctrl_out(&mut e, 's'), InsertOutcome::Stay));
}

// ── Normalization on every save ──────────────────────────────────────

fn save_text(e: &mut MarkdownEditor) -> String {
    match ctrl_out(e, 's') {
        InsertOutcome::Save(Command::EditCardDescription { body, .. }) => body,
        _ => panic!("expected a save"),
    }
}

#[test]
fn every_save_normalizes_without_paste_mode() {
    let mut e = make_editor("5. a\n9. b\n    - deep\n|a|b|\n|-|-|\n|1|2|");
    plain(&mut e, '!');
    assert!(!e.paste_mode());
    let body = save_text(&mut e);
    let lines: Vec<&str> = body.lines().collect();
    assert_eq!(&lines[..2], ["5. a", "6. b"]);
    assert_eq!(lines[2], "   - deep");
    assert!(lines[4].starts_with("| ---"), "{lines:?}");
}

#[test]
fn ctrl_q_normalizes_too_and_in_one_undo_step() {
    let mut e = make_editor("1. a\n  - b");
    plain(&mut e, 'x');
    let out = ctrl_out(&mut e, 'q');
    assert!(matches!(
        out,
        InsertOutcome::Confirm(Command::EditCardDescription { ref body, .. }) if body == "1. a\n   - bx"
    ));
    ctrl(&mut e, 'z');
    assert_eq!(editor_lines(&e)[1], "  - bx");
}

#[test]
fn fenced_code_is_not_normalized_on_save() {
    let mut e = make_editor("```\n5. keep\n9. keep\n```");
    plain(&mut e, 'x');
    let body = save_text(&mut e);
    assert!(body.contains("5. keep\n9. keep"), "{body}");
}

#[test]
fn untouched_text_is_not_normalized_or_saved() {
    let mut e = make_editor("5. a\n9. b");
    assert!(matches!(ctrl_out(&mut e, 's'), InsertOutcome::Stay));
    assert!(matches!(ctrl_out(&mut e, 'q'), InsertOutcome::Cancel));
    assert_eq!(editor_lines(&e), ["5. a", "9. b"]);
}

#[test]
fn saved_text_is_stable_under_a_second_normalization() {
    let mut e = make_editor("2. a\n   7. b\n 9. c");
    plain(&mut e, 'x');
    let first = save_text(&mut e);
    plain(&mut e, 'y');
    let second = save_text(&mut e);
    assert_eq!(first.lines().count(), second.lines().count());
    assert_eq!(
        second.replace('y', "").replace("x", ""),
        first.replace('x', "")
    );
}

#[test]
fn numbered_lines_in_fences_keep_their_numbers_when_editing_around_them() {
    let mut e = make_editor("```\n5. x\n```\n3. a");
    cursor_at(&mut e, 3, 4);
    press(&mut e, KeyCode::Enter);
    assert_eq!(editor_lines(&e)[1], "5. x");
}
