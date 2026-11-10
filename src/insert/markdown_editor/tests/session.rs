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
        InsertOutcome::Save(Command::EditCardDescription { card_id, body }) => {
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
        InsertOutcome::Confirm(Command::EditCardDescription { card_id, body }) => {
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
