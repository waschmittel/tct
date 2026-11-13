use super::*;

#[test]
fn history_records_field_with_old_and_new_values() {
    with_temp_dir(|| {
        let (mut ed, _, ids) = editor_with_cards(&["t"]);
        let id = ids[0].clone();
        ed.apply(Command::DefineLabel {
            name: "foo".into(),
            color: crate::model::label::LabelColor::generate_pastel(&[]),
        })
        .unwrap();
        let label_id = ed.board().meta.labels[0].id.clone();
        ed.apply(Command::ToggleLabel {
            card_id: id.clone(),
            label_id,
        })
        .unwrap();
        ed.apply(Command::EditCardTitle {
            card_id: id.clone(),
            title: "u".into(),
        })
        .unwrap();
        ed.apply(Command::AddChecklistItem {
            card_id: id.clone(),
            text: "x".into(),
        })
        .unwrap();
        ed.reload().unwrap();

        let h = &ed.board().cards[&id].history;
        assert_eq!(h[0].field, HistoryField::Labels);
        assert_eq!(h[0].old.as_deref(), Some(""));
        assert_eq!(h[0].new.as_deref(), Some("foo"));
        assert_eq!(h[1].field, HistoryField::Title);
        assert_eq!(
            (h[1].old.as_deref(), h[1].new.as_deref()),
            (Some("t"), Some("u"))
        );
        assert_eq!(h[2].field, HistoryField::Checklist);
        assert_eq!(h[2].old.as_deref(), Some(""));
        assert_eq!(h[2].new.as_deref(), Some("- [ ] x"));
    });
}

#[test]
fn delete_history_entry_removes_only_that_entry() {
    with_temp_dir(|| {
        let (mut ed, _, ids) = editor_with_cards(&["t"]);
        let id = ids[0].clone();
        ed.apply(Command::EditCardTitle {
            card_id: id.clone(),
            title: "a".into(),
        })
        .unwrap();
        ed.apply(Command::EditCardTitle {
            card_id: id.clone(),
            title: "b".into(),
        })
        .unwrap();
        ed.apply(Command::DeleteHistoryEntry {
            card_id: id.clone(),
            index: 0,
        })
        .unwrap();
        ed.reload().unwrap();
        let h = &ed.board().cards[&id].history;
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].new.as_deref(), Some("b"));
        assert!(
            ed.apply(Command::DeleteHistoryEntry {
                card_id: id,
                index: 5
            })
            .is_err()
        );
    });
}

#[test]
fn history_records_description_and_due_date_changes() {
    with_temp_dir(|| {
        let (mut ed, _, ids) = editor_with_cards(&["t"]);
        let id = ids[0].clone();
        let d1 = chrono::NaiveDate::from_ymd_opt(2099, 1, 1).unwrap();
        let d2 = chrono::NaiveDate::from_ymd_opt(2099, 2, 2).unwrap();
        ed.apply(Command::EditCardDescription {
            card_id: id.clone(),
            body: "hi".into(),
            cursor: None,
        })
        .unwrap();
        ed.apply(Command::SetDueDate {
            card_id: id.clone(),
            date: d1,
        })
        .unwrap();
        ed.apply(Command::SetDueDate {
            card_id: id.clone(),
            date: d2,
        })
        .unwrap();
        ed.apply(Command::ClearDueDate {
            card_id: id.clone(),
        })
        .unwrap();
        let h = &ed.board().cards[&id].history;
        assert_eq!(h.len(), 4);
        assert_eq!(h[0].field, HistoryField::Description);
        assert_eq!(
            (h[0].old.as_deref(), h[0].new.as_deref()),
            (Some(""), Some("hi"))
        );
        assert_eq!(
            (h[1].old.as_deref(), h[1].new.as_deref()),
            (None, Some("2099-01-01"))
        );
        assert_eq!(
            (h[2].old.as_deref(), h[2].new.as_deref()),
            (Some("2099-01-01"), Some("2099-02-02"))
        );
        assert_eq!(h[3].field, HistoryField::DueDate);
        assert_eq!(
            (h[3].old.as_deref(), h[3].new.as_deref()),
            (Some("2099-02-02"), None)
        );
    });
}

#[test]
fn unchanged_values_add_no_history_entry() {
    with_temp_dir(|| {
        let (mut ed, _, ids) = editor_with_cards(&["t"]);
        let id = ids[0].clone();
        ed.apply(Command::EditCardTitle {
            card_id: id.clone(),
            title: "t".into(),
        })
        .unwrap();
        ed.apply(Command::EditCardDescription {
            card_id: id.clone(),
            body: String::new(),
            cursor: None,
        })
        .unwrap();
        ed.apply(Command::ClearDueDate {
            card_id: id.clone(),
        })
        .unwrap();
        assert!(ed.board().cards[&id].history.is_empty());
    });
}

#[test]
fn history_records_every_checklist_mutation() {
    with_temp_dir(|| {
        let (mut ed, _, ids) = editor_with_cards(&["t"]);
        let id = ids[0].clone();
        for text in ["a", "b"] {
            ed.apply(Command::AddChecklistItem {
                card_id: id.clone(),
                text: text.into(),
            })
            .unwrap();
        }
        ed.apply(Command::ToggleChecklistItem {
            card_id: id.clone(),
            item_idx: 0,
        })
        .unwrap();
        ed.apply(Command::EditChecklistItem {
            card_id: id.clone(),
            item_idx: 1,
            text: "c".into(),
        })
        .unwrap();
        ed.apply(Command::ReorderChecklistItem {
            card_id: id.clone(),
            from: 1,
            to: 0,
        })
        .unwrap();
        ed.apply(Command::RemoveChecklistItem {
            card_id: id.clone(),
            item_idx: 0,
        })
        .unwrap();
        let h = &ed.board().cards[&id].history;
        assert_eq!(h.len(), 6);
        assert!(h.iter().all(|e| e.field == HistoryField::Checklist));
        let pair = |i: usize| (h[i].old.clone().unwrap(), h[i].new.clone().unwrap());
        assert_eq!(
            pair(2),
            ("- [ ] a\n- [ ] b".into(), "- [x] a\n- [ ] b".into())
        );
        assert_eq!(
            pair(3),
            ("- [x] a\n- [ ] b".into(), "- [x] a\n- [ ] c".into())
        );
        assert_eq!(
            pair(4),
            ("- [x] a\n- [ ] c".into(), "- [ ] c\n- [x] a".into())
        );
        assert_eq!(pair(5), ("- [ ] c\n- [x] a".into(), "- [x] a".into()));
    });
}

#[test]
fn untoggling_label_records_removal() {
    with_temp_dir(|| {
        let (mut ed, _, ids) = editor_with_cards(&["t"]);
        let id = ids[0].clone();
        ed.apply(Command::DefineLabel {
            name: "foo".into(),
            color: crate::model::label::LabelColor::generate_pastel(&[]),
        })
        .unwrap();
        let label_id = ed.board().meta.labels[0].id.clone();
        for _ in 0..2 {
            ed.apply(Command::ToggleLabel {
                card_id: id.clone(),
                label_id: label_id.clone(),
            })
            .unwrap();
        }
        let h = &ed.board().cards[&id].history;
        assert_eq!(
            (h[1].old.as_deref(), h[1].new.as_deref()),
            (Some("foo"), Some(""))
        );
        assert_eq!(h[1].action, "Removed label 'foo'");
    });
}

#[test]
fn date_stamp_commands_set_independently_and_persist() {
    with_temp_dir(|| {
        let (mut ed, _, _) = editor_with_cards(&["t"]);
        ed.apply(Command::SetShowCreatedDate { show: true })
            .unwrap();
        assert!(ed.board().meta.show_created_date && !ed.board().meta.show_updated_date);
        ed.apply(Command::SetShowUpdatedDate { show: true })
            .unwrap();
        ed.apply(Command::SetShowCreatedDate { show: false })
            .unwrap();
        ed.reload().unwrap();
        assert!(!ed.board().meta.show_created_date && ed.board().meta.show_updated_date);
    });
}

#[test]
fn delete_history_entry_unknown_card_errors() {
    with_temp_dir(|| {
        let (mut ed, _, _) = editor_with_cards(&["t"]);
        assert!(
            ed.apply(Command::DeleteHistoryEntry {
                card_id: "nope".into(),
                index: 0
            })
            .is_err()
        );
    });
}

#[test]
fn the_description_cursor_is_never_part_of_history() {
    with_temp_dir(|| {
        let (mut ed, _, ids) = editor_with_cards(&["t"]);
        let id = ids[0].clone();
        let save = |ed: &mut _, body: &str, cursor| {
            let ed: &mut crate::board_editor::BoardEditor = ed;
            ed.apply(Command::EditCardDescription {
                card_id: id.clone(),
                body: body.into(),
                cursor,
            })
            .unwrap();
        };
        let entries = |ed: &crate::board_editor::BoardEditor| ed.board().cards[&id].history.len();
        let before = entries(&ed);
        save(&mut ed, "hi", Some((0, 2)));
        assert_eq!(entries(&ed), before + 1);
        // Only the cursor moved: no entry at all.
        save(&mut ed, "hi", Some((0, 0)));
        save(&mut ed, "hi", None);
        assert_eq!(entries(&ed), before + 1);
        let last = ed.board().cards[&id].history.last().unwrap();
        assert_eq!(last.old.as_deref(), Some(""));
        assert_eq!(last.new.as_deref(), Some("hi"));
        assert!(!last.action.contains('('));
    });
}
