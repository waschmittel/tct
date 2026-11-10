use super::*;

#[test]
fn ctrl_n_adds_a_reference_and_a_definition_at_the_bottom() {
    let mut e = make_editor("Hello");
    footnote_key(&mut e);
    assert_eq!(editor_lines(&e), ["Hello[^1]", "", "[^1]: "]);
    assert_eq!(editor_cursor(&e), (2, 6));
    // The cursor is ready to type the note.
    e.handle_paste("My note");
    assert_eq!(editor_lines(&e)[2], "[^1]: My note");
}

#[test]
fn ctrl_n_counts_up_and_stacks_definitions_without_blank_lines() {
    let mut e = make_editor("a b\n\nmore");
    cursor_at(&mut e, 0, 1);
    footnote_key(&mut e);
    cursor_at(&mut e, 2, 4);
    footnote_key(&mut e);
    cursor_at(&mut e, 0, 0);
    footnote_key(&mut e);
    assert_eq!(
        editor_lines(&e),
        [
            "[^1]a[^2] b",
            "",
            "more[^3]",
            "",
            "[^1]: ",
            "[^2]: ",
            "[^3]: "
        ]
    );
    assert_eq!(editor_cursor(&e), (4, 6));
}

#[test]
fn ctrl_n_renumbers_numbered_footnotes_in_ascending_order_of_reference() {
    let mut e = make_editor("x[^7] y[^name]\n\n[^7]: seven\n[^name]: n");
    cursor_at(&mut e, 0, 0);
    footnote_key(&mut e);
    assert_eq!(
        editor_lines(&e),
        [
            "[^1]x[^2] y[^name]",
            "",
            "[^1]: ",
            "[^2]: seven",
            "[^name]: n"
        ]
    );
}

#[test]
fn ctrl_n_between_references_shifts_the_later_ones_and_keeps_the_cursor_in_place() {
    let mut e = make_editor("a[^1] b[^2] c[^3]\n\n[^1]: one\n[^2]: two\n[^3]: three");
    cursor_at(&mut e, 0, 6);
    footnote_key(&mut e);
    assert_eq!(
        editor_lines(&e),
        [
            "a[^1] [^2]b[^3] c[^4]",
            "",
            "[^1]: one",
            "[^2]: ",
            "[^3]: two",
            "[^4]: three"
        ]
    );
    assert_eq!(editor_cursor(&e), (3, 6));
    // Jumping back lands behind the new reference.
    footnote_key(&mut e);
    assert_eq!(editor_cursor(&e), (0, 10));
}

#[test]
fn ctrl_n_closes_gaps_and_leaves_names_and_code_alone() {
    let mut e = make_editor("a[^5] [^n]\n```\n[^5]\n```\n\n[^5]: five\n[^n]: name");
    cursor_at(&mut e, 0, 0);
    footnote_key(&mut e);
    let lines = editor_lines(&e);
    assert_eq!(lines[0], "[^1]a[^2] [^n]");
    assert_eq!(lines[2], "[^5]");
    assert_eq!(&lines[5..], ["[^1]: ", "[^2]: five", "[^n]: name"]);
}

#[test]
fn ctrl_n_replaces_trailing_blank_lines_and_is_one_undo_step() {
    let mut e = make_editor("a\n\n\n");
    cursor_at(&mut e, 0, 1);
    footnote_key(&mut e);
    assert_eq!(editor_lines(&e), ["a[^1]", "", "[^1]: "]);
    e.input.textarea.undo();
    assert_eq!(editor_lines(&e), ["a", "", "", ""]);
}

#[test]
fn ctrl_n_after_a_multi_line_definition_still_stacks() {
    let mut e = make_editor("a[^1]\n\n[^1]: one  \nsecond line");
    cursor_at(&mut e, 0, 0);
    footnote_key(&mut e);
    assert_eq!(
        editor_lines(&e),
        ["[^1]a[^2]", "", "[^1]: ", "[^2]: one  ", "second line"]
    );
}

#[test]
fn ctrl_n_in_a_definition_jumps_back_to_the_first_reference() {
    let mut e = make_editor("a[^2] b[^1] c[^1]\n\n[^1]: one  \nsecond\n[^2]: two");
    cursor_at(&mut e, 2, 8);
    footnote_key(&mut e);
    assert_eq!(editor_cursor(&e), (0, 11));
    // Also from a continuation line, and for the other label.
    cursor_at(&mut e, 3, 2);
    footnote_key(&mut e);
    assert_eq!(editor_cursor(&e), (0, 11));
    cursor_at(&mut e, 4, 0);
    footnote_key(&mut e);
    assert_eq!(editor_cursor(&e), (0, 5));
}

#[test]
fn ctrl_n_in_an_unreferenced_definition_reports_and_stays() {
    let mut e = make_editor("text\n\n[^x]: orphan");
    cursor_at(&mut e, 2, 3);
    let out = footnote_key(&mut e);
    assert!(matches!(out, InsertOutcome::StayWithStatus(Some(m)) if m.contains("[^x]")));
    assert_eq!(editor_cursor(&e), (2, 3));
    assert_eq!(editor_lines(&e).len(), 3);
}

#[test]
fn ctrl_n_in_a_table_cell_reflows_the_table() {
    let mut e = make_editor("| a | b |\n|---|---|\n| 1 | 2 |");
    cursor_at(&mut e, 2, 3);
    footnote_key(&mut e);
    let lines = editor_lines(&e);
    assert!(lines[2].contains("1[^1]"), "{lines:?}");
    assert_eq!(lines[0].len(), lines[2].len());
    assert_eq!(lines.last().unwrap(), "[^1]: ");
}

#[test]
fn ctrl_n_names_the_footnote_after_highlighted_text_and_keeps_the_text() {
    let mut e = make_editor("hello big world");
    cursor_at(&mut e, 0, 6);
    e.input.textarea.start_selection();
    e.input.textarea.move_cursor(CursorMove::Jump(0, 9));
    footnote_key(&mut e);
    assert_eq!(editor_lines(&e), ["hello big[^big] world", "", "[^big]: "]);
    assert_eq!(editor_cursor(&e), (2, 8));
    assert!(!e.input.textarea.is_selecting());
    // Back from the definition lands behind the reference.
    footnote_key(&mut e);
    assert_eq!(editor_cursor(&e), (0, 15));
}

#[test]
fn highlighted_words_and_lines_become_a_dashed_label_and_backward_selection_works() {
    let mut e = make_editor("two words\nnext line");
    select(&mut e, (0, 0), (1, 4));
    footnote_key(&mut e);
    assert_eq!(editor_lines(&e)[1], "next[^two-words-next] line");
    assert_eq!(editor_lines(&e).last().unwrap(), "[^two-words-next]: ");

    let mut e = make_editor("alpha beta");
    select(&mut e, (0, 5), (0, 0)); // cursor before the anchor
    footnote_key(&mut e);
    assert_eq!(editor_lines(&e)[0], "alpha[^alpha] beta");
}

#[test]
fn highlighted_text_without_a_usable_name_falls_back_to_numbering() {
    for text in ["12", "[^]", "   "] {
        let mut e = make_editor(&format!("x {text} y"));
        select(&mut e, (0, 2), (0, 2 + text.len()));
        footnote_key(&mut e);
        let lines = editor_lines(&e);
        assert!(lines[0].contains("[^1]"), "{text}: {lines:?}");
        assert_eq!(lines.last().unwrap(), "[^1]: ", "{text}");
    }
    // An empty selection is no selection.
    let mut e = make_editor("x");
    select(&mut e, (0, 1), (0, 1));
    footnote_key(&mut e);
    assert_eq!(editor_lines(&e), ["x[^1]", "", "[^1]: "]);
}

#[test]
fn a_name_that_is_already_defined_jumps_to_its_definition_without_a_second_one() {
    let mut e = make_editor("see term and term\n\n[^term]: Defined.");
    select(&mut e, (0, 13), (0, 17));
    footnote_key(&mut e);
    assert_eq!(
        editor_lines(&e),
        ["see term and term[^term]", "", "[^term]: Defined."]
    );
    assert_eq!(editor_cursor(&e), (2, 17));
}

#[test]
fn named_footnotes_do_not_renumber_the_numbered_ones() {
    let mut e = make_editor("a[^2] b[^5] word");
    select(&mut e, (0, 12), (0, 16));
    footnote_key(&mut e);
    assert_eq!(editor_lines(&e)[0], "a[^2] b[^5] word[^word]");
}

#[test]
fn highlighted_text_in_a_table_cell_names_the_footnote_and_reflows() {
    let mut e = make_editor("| a | b |\n|---|---|\n| one | 2 |");
    select(&mut e, (2, 2), (2, 5));
    footnote_key(&mut e);
    let lines = editor_lines(&e);
    assert!(lines[2].contains("one[^one]"), "{lines:?}");
    assert_eq!(lines[0].len(), lines[2].len());
}

#[test]
fn ctrl_n_works_in_paste_mode() {
    let mut e = make_editor("x");
    paste_mode_on(&mut e);
    footnote_key(&mut e);
    assert_eq!(editor_lines(&e), ["x[^1]", "", "[^1]: "]);
}

#[test]
fn a_numbered_definition_goes_where_its_number_belongs() {
    let mut e = make_editor("a[^1] b[^3]\n\n[^1]: one  \nmore\n\n    para\n[^3]: three");
    cursor_at(&mut e, 0, 6);
    footnote_key(&mut e);
    assert_eq!(
        editor_lines(&e),
        [
            "a[^1] [^2]b[^3]",
            "",
            "[^1]: one  ",
            "more",
            "",
            "    para",
            "[^2]: ",
            "[^3]: three"
        ]
    );
    assert_eq!(editor_cursor(&e), (6, 6));
}

#[test]
fn definitions_spread_through_the_text_get_the_new_one_behind_its_predecessor() {
    let mut e = make_editor("a b\n\n[^1]: one\n\ntext\n\n[^3]: three\n\nend");
    cursor_at(&mut e, 0, 1);
    footnote_key(&mut e);
    // [^1] and [^3] are unreferenced, so they keep their numbers.
    assert_eq!(
        editor_lines(&e),
        [
            "a[^2] b",
            "",
            "[^1]: one",
            "[^2]: ",
            "",
            "text",
            "",
            "[^3]: three",
            "",
            "end"
        ]
    );
}

#[test]
fn named_definitions_do_not_take_part_in_the_order() {
    let mut e = make_editor("x[^1]\n\n[^note]: n\n[^1]: one");
    cursor_at(&mut e, 0, 0);
    footnote_key(&mut e);
    assert_eq!(
        editor_lines(&e),
        ["[^1]x[^2]", "", "[^note]: n", "[^1]: ", "[^2]: one"]
    );
}

#[test]
fn a_footnote_named_by_selection_still_goes_to_the_bottom() {
    let mut e = make_editor("x[^1] word\n\n[^1]: one\n\ntail");
    select(&mut e, (0, 6), (0, 10));
    footnote_key(&mut e);
    let lines = editor_lines(&e);
    assert_eq!(lines[0], "x[^1] word[^word]");
    assert_eq!(&lines[4..], ["tail", "", "[^word]: "]);
}

#[test]
fn mismatched_references_and_definitions_keep_resolving_to_the_same_footnotes() {
    // Three references, one definition: only [^2] is a pair.
    let mut e = make_editor("a[^1] b[^2] c[^3]\n\n[^2]: two");
    cursor_at(&mut e, 0, 0);
    footnote_key(&mut e);
    assert_eq!(
        editor_lines(&e),
        ["[^2]a[^1] b[^4] c[^3]", "", "[^2]: ", "[^4]: two"]
    );
    // Dangling references stay dangling, and no new number landed on them.
    assert_eq!(
        targets(&editor_lines(&e)),
        [Some("".into()), None, Some("two".into()), None]
    );

    // An unreferenced definition and a dangling reference.
    let mut e = make_editor("a[^5] b[^1]\n\n[^5]: five");
    cursor_at(&mut e, 0, 0);
    footnote_key(&mut e);
    assert_eq!(
        editor_lines(&e),
        ["[^2]a[^3] b[^1]", "", "[^2]: ", "[^3]: five"]
    );
    assert_eq!(
        targets(&editor_lines(&e)),
        [Some("".into()), Some("five".into()), None]
    );
}

#[test]
fn a_footnote_inserted_among_others_renumbers_and_places_in_one_undo_step() {
    let doc = "a[^1] b[^2]\n\n[^1]: one\n[^2]: two";
    let mut e = make_editor(doc);
    cursor_at(&mut e, 0, 6);
    footnote_key(&mut e);
    assert_eq!(editor_lines(&e).len(), 5);
    e.input.textarea.undo();
    assert_eq!(e.input.current_text(), doc);
}

#[test]
fn a_cursor_inside_a_reference_inserts_behind_it() {
    let mut e = make_editor("x[^1]y\n\n[^1]: one");
    cursor_at(&mut e, 0, 3);
    footnote_key(&mut e);
    assert_eq!(editor_lines(&e)[0], "x[^1][^2]y");
}

#[test]
fn several_references_to_one_footnote_stay_together() {
    let mut e = make_editor("a[^1] b[^1] c[^1]\n\n[^1]: one");
    cursor_at(&mut e, 0, 6);
    footnote_key(&mut e);
    assert_eq!(
        editor_lines(&e),
        ["a[^1] [^2]b[^1] c[^1]", "", "[^1]: one", "[^2]: "]
    );
}

#[test]
fn inserting_anywhere_never_redirects_an_existing_reference() {
    let docs = [
        "a[^1] b[^2] c[^3]\n\n[^2]: two",
        "a[^5] b[^1]\n\n[^5]: five\n[^3]: orphan",
        "a[^2] b[^2]\nc[^1] d[^7]\n\n[^1]: one[^2]\n[^2]: two\n[^9]: nine",
        "[^3]: t\nx[^3] y[^4]\n`[^3]`\n```\n[^3]\n```\n[^n]: name z[^n]",
        "plain text without footnotes",
    ];
    for doc in docs {
        let lines: Vec<String> = doc.lines().map(String::from).collect();
        let before = targets(&lines);
        let fenced = footnote::fenced_rows(&lines);
        for row in 0..lines.len() {
            // In a definition Ctrl+N jumps; in code a reference is literal.
            if fenced[row] || footnote::definition_at(&lines, row).is_some() {
                continue;
            }
            for col in 0..=lines[row].chars().count() {
                let mut e = make_editor(doc);
                cursor_at(&mut e, row, col);
                footnote_key(&mut e);
                let after = editor_lines(&e);
                let mut expected = before.clone();
                // The new reference points at the new, empty definition.
                expected.insert(refs_before(&lines, row, col), Some(String::new()));
                assert_eq!(
                    targets(&after),
                    expected,
                    "{doc:?} at ({row},{col}): {after:?}"
                );
            }
        }
    }
}

#[test]
fn repeated_ctrl_n_keeps_numbered_definitions_in_order() {
    let mut e = make_editor("one two three four five");
    for col in [3, 7, 13, 18] {
        cursor_at(&mut e, 0, col);
        footnote_key(&mut e);
        e.handle_paste("note");
        // Back in the text for the next one, shifted by the references so far.
    }
    let lines = editor_lines(&e);
    let defs: Vec<&String> = lines.iter().filter(|l| l.starts_with("[^")).collect();
    assert_eq!(defs.len(), 4);
    let numbers: Vec<u64> = defs
        .iter()
        .map(|l| footnote::def_start(l).unwrap().0.parse().unwrap())
        .collect();
    assert!(numbers.windows(2).all(|w| w[0] < w[1]), "{lines:?}");
}
