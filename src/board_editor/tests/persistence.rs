use super::*;

#[test]
fn new_card_position_command_persists_across_reload() {
    with_temp_dir(|| {
        let (mut ed, _, _) = editor_with_cards(&["t"]);
        assert_eq!(
            ed.board().meta.new_card_position,
            crate::model::board::NewCardPosition::Bottom
        );
        ed.apply(Command::SetNewCardPosition {
            position: crate::model::board::NewCardPosition::Top,
        })
        .unwrap();
        assert_eq!(
            ed.board().meta.new_card_position,
            crate::model::board::NewCardPosition::Top
        );
        ed.reload().unwrap();
        assert_eq!(
            ed.board().meta.new_card_position,
            crate::model::board::NewCardPosition::Top
        );
    });
}

#[test]
fn add_card_top_inserts_at_beginning_and_persists() {
    with_temp_dir(|| {
        let (mut ed, lid, _) = editor_with_cards(&["existing1", "existing2"]);
        ed.apply(Command::SetNewCardPosition {
            position: crate::model::board::NewCardPosition::Top,
        })
        .unwrap();
        ed.apply(Command::AddCard {
            list_id: lid.clone(),
            title: "fresh_top".into(),
        })
        .unwrap();
        let new_id = ed.last_added_card_id().cloned().unwrap();
        assert_eq!(ed.board().lists[0].card_ids.first(), Some(&new_id));
        assert_eq!(ed.board().selected_card[0], 0);

        ed.reload().unwrap();
        assert_eq!(ed.board().cards[&new_id].title, "fresh_top");
        assert_eq!(ed.board().lists[0].card_ids.first(), Some(&new_id));

        // Add another one at top
        ed.apply(Command::AddCard {
            list_id: lid.clone(),
            title: "even_newer".into(),
        })
        .unwrap();
        let newest_id = ed.last_added_card_id().cloned().unwrap();
        assert_eq!(ed.board().lists[0].card_ids.first(), Some(&newest_id));
        assert_eq!(ed.board().lists[0].card_ids[1], new_id);
        assert_eq!(ed.board().selected_card[0], 0);
    });
}

#[test]
fn add_card_persists_across_reload() {
    with_temp_dir(|| {
        let (mut ed, lid, _) = editor_with_cards(&["existing"]);
        ed.apply(Command::AddCard {
            list_id: lid.clone(),
            title: "fresh".into(),
        })
        .unwrap();
        let new_id = ed.last_added_card_id().cloned().unwrap();
        ed.reload().unwrap();
        assert_eq!(ed.board().cards[&new_id].title, "fresh");
        assert_eq!(ed.board().cards[&new_id].list_id, lid);
        // Ordered after the existing card.
        assert_eq!(ed.board().lists[0].card_ids.last(), Some(&new_id));
    });
}

#[test]
fn edit_title_and_description_persist_across_reload() {
    with_temp_dir(|| {
        let (mut ed, _, c) = editor_with_cards(&["orig"]);
        let id = c[0].clone();
        ed.apply(Command::EditCardTitle {
            card_id: id.clone(),
            title: "renamed".into(),
        })
        .unwrap();
        ed.apply(Command::EditCardDescription {
            card_id: id.clone(),
            body: "body text".into(),
        })
        .unwrap();
        ed.reload().unwrap();
        assert_eq!(ed.board().cards[&id].title, "renamed");
        assert_eq!(ed.board().cards[&id].description, "body text");
    });
}

#[test]
fn due_date_set_then_clear_persists_across_reload() {
    with_temp_dir(|| {
        let (mut ed, _, c) = editor_with_cards(&["task"]);
        let id = c[0].clone();
        let date = chrono::NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
        ed.apply(Command::SetDueDate {
            card_id: id.clone(),
            date,
        })
        .unwrap();
        ed.reload().unwrap();
        assert_eq!(ed.board().cards[&id].due_date, Some(date));
        ed.apply(Command::ClearDueDate {
            card_id: id.clone(),
        })
        .unwrap();
        ed.reload().unwrap();
        assert_eq!(ed.board().cards[&id].due_date, None);
    });
}

#[test]
fn archive_then_restore_card_persists_across_reload() {
    with_temp_dir(|| {
        let (mut ed, lid, c) = editor_with_cards(&["task"]);
        let id = c[0].clone();
        ed.apply(Command::ArchiveCard {
            card_id: id.clone(),
        })
        .unwrap();
        ed.reload().unwrap();
        assert!(ed.board().cards[&id].archived);
        ed.apply(Command::RestoreCard {
            card_id: id.clone(),
        })
        .unwrap();
        ed.reload().unwrap();
        assert!(!ed.board().cards[&id].archived);
        // Still a member of its original list, hence visible again.
        assert_eq!(ed.board().cards[&id].list_id, lid);
        assert!(ed.board().lists[0].card_ids.contains(&id));
    });
}

#[test]
fn checklist_lifecycle_persists_across_reload() {
    with_temp_dir(|| {
        let (mut ed, _, c) = editor_with_cards(&["task"]);
        let id = c[0].clone();
        ed.apply(Command::AddChecklistItem {
            card_id: id.clone(),
            text: "one".into(),
        })
        .unwrap();
        ed.apply(Command::AddChecklistItem {
            card_id: id.clone(),
            text: "two".into(),
        })
        .unwrap();
        ed.apply(Command::AddChecklistItem {
            card_id: id.clone(),
            text: "three".into(),
        })
        .unwrap();
        ed.apply(Command::ToggleChecklistItem {
            card_id: id.clone(),
            item_idx: 0,
        })
        .unwrap();
        ed.apply(Command::EditChecklistItem {
            card_id: id.clone(),
            item_idx: 1,
            text: "TWO".into(),
        })
        .unwrap();
        ed.apply(Command::ReorderChecklistItem {
            card_id: id.clone(),
            from: 2,
            to: 0,
        })
        .unwrap();
        ed.reload().unwrap();
        let cl = &ed.board().cards[&id].checklist;
        let names: Vec<&str> = cl.iter().map(|i| i.text.as_str()).collect();
        assert_eq!(names, vec!["three", "one", "TWO"], "order after reorder");
        // "one" was completed; it is now at index 1 after the reorder.
        assert!(cl[1].completed);
        ed.apply(Command::RemoveChecklistItem {
            card_id: id.clone(),
            item_idx: 0,
        })
        .unwrap();
        ed.reload().unwrap();
        let names: Vec<&str> = ed.board().cards[&id]
            .checklist
            .iter()
            .map(|i| i.text.as_str())
            .collect();
        assert_eq!(names, vec!["one", "TWO"]);
    });
}

#[test]
fn toggle_label_assignment_persists_across_reload() {
    with_temp_dir(|| {
        let (mut ed, _, c) = editor_with_cards(&["task"]);
        let id = c[0].clone();
        ed.apply(Command::DefineLabel {
            name: "urgent".into(),
            color: LabelColor::Red,
        })
        .unwrap();
        let lbl = ed.board().meta.labels[0].id.clone();
        ed.apply(Command::ToggleLabel {
            card_id: id.clone(),
            label_id: lbl.clone(),
        })
        .unwrap();
        ed.reload().unwrap();
        assert_eq!(ed.board().cards[&id].label_ids, vec![lbl.clone()]);
        ed.apply(Command::ToggleLabel {
            card_id: id.clone(),
            label_id: lbl.clone(),
        })
        .unwrap();
        ed.reload().unwrap();
        assert!(ed.board().cards[&id].label_ids.is_empty());
    });
}

#[test]
fn delete_label_prunes_from_card_on_next_save() {
    with_temp_dir(|| {
        let (mut ed, _, c) = editor_with_cards(&["task"]);
        let id = c[0].clone();
        ed.apply(Command::DefineLabel {
            name: "temp".into(),
            color: LabelColor::Green,
        })
        .unwrap();
        let lbl = ed.board().meta.labels[0].id.clone();
        ed.apply(Command::ToggleLabel {
            card_id: id.clone(),
            label_id: lbl.clone(),
        })
        .unwrap();
        ed.apply(Command::DeleteLabel {
            label_id: lbl.clone(),
        })
        .unwrap();
        // Lazy prune: the dead id is shed on the card's next save.
        ed.apply(Command::EditCardTitle {
            card_id: id.clone(),
            title: "touched".into(),
        })
        .unwrap();
        ed.reload().unwrap();
        assert!(ed.board().meta.labels.is_empty());
        assert!(
            ed.board().cards[&id].label_ids.is_empty(),
            "dead label id pruned on save"
        );
    });
}

#[test]
fn label_meta_commands_persist_across_reload() {
    with_temp_dir(|| {
        let (mut ed, _, _) = editor_with_cards(&["task"]);
        ed.apply(Command::DefineLabel {
            name: "a".into(),
            color: LabelColor::Red,
        })
        .unwrap();
        ed.apply(Command::DefineLabel {
            name: "b".into(),
            color: LabelColor::Green,
        })
        .unwrap();
        let a = ed.board().meta.labels[0].id.clone();
        ed.apply(Command::RenameLabel {
            label_id: a.clone(),
            name: "alpha".into(),
        })
        .unwrap();
        ed.apply(Command::SetLabelColor {
            label_id: a.clone(),
            color: LabelColor::Blue,
        })
        .unwrap();
        ed.apply(Command::ReorderLabels { from: 0, to: 1 }).unwrap();
        ed.reload().unwrap();
        // After reorder, "alpha" (the renamed/recolored one) is last.
        let labels = &ed.board().meta.labels;
        assert_eq!(labels.len(), 2);
        assert_eq!(labels[1].name, "alpha");
        assert_eq!(labels[1].color, LabelColor::Blue);
        assert_eq!(labels[0].name, "b");
    });
}

#[test]
fn list_commands_persist_across_reload() {
    with_temp_dir(|| {
        let (mut ed, lid, _) = editor_with_cards(&["task"]);
        ed.apply(Command::AddList {
            name: "Second".into(),
        })
        .unwrap();
        ed.apply(Command::RenameList {
            list_id: lid.clone(),
            name: "First".into(),
        })
        .unwrap();
        ed.reload().unwrap();
        assert_eq!(ed.board().lists.len(), 2);
        assert_eq!(ed.board().lists[0].name, "First");
        assert_eq!(ed.board().lists[1].name, "Second");

        // Reorder: move "First" right (down the meta order).
        ed.apply(Command::MoveList {
            list_id: lid.clone(),
            direction: MoveDir::Right,
        })
        .unwrap();
        ed.reload().unwrap();
        assert_eq!(ed.board().lists[0].name, "Second");
        assert_eq!(ed.board().lists[1].name, "First");
    });
}

#[test]
fn archive_then_restore_list_persists_across_reload() {
    with_temp_dir(|| {
        let (mut ed, lid, _) = editor_with_cards(&["task"]);
        ed.apply(Command::AddList {
            name: "Keep".into(),
        })
        .unwrap();
        ed.apply(Command::ArchiveList {
            list_id: lid.clone(),
        })
        .unwrap();
        ed.reload().unwrap();
        // Active view excludes the archived list.
        assert_eq!(ed.board().lists.len(), 1);
        assert_eq!(ed.board().lists[0].name, "Keep");

        ed.apply(Command::RestoreList {
            list_id: lid.clone(),
        })
        .unwrap();
        ed.reload().unwrap();
        assert_eq!(ed.board().lists.len(), 2);
        assert!(ed.board().lists.iter().any(|l| l.id == lid));
    });
}

#[test]
fn board_commands_persist_across_reload() {
    with_temp_dir(|| {
        let (mut ed, _, _) = editor_with_cards(&["task"]);
        let bid = ed.board().meta.id.clone();
        ed.apply(Command::RenameBoard {
            name: "Renamed Board".into(),
        })
        .unwrap();
        ed.apply(Command::SetAccentColor {
            color: LabelColor::Blue,
        })
        .unwrap();
        ed.reload().unwrap();
        assert_eq!(ed.board().meta.name, "Renamed Board");
        assert_eq!(ed.board().meta.accent_color, LabelColor::Blue);

        ed.apply(Command::ArchiveBoard {
            board_id: bid.clone(),
        })
        .unwrap();
        ed.reload().unwrap();
        assert!(ed.board().meta.archived);
        ed.apply(Command::RestoreBoard {
            board_id: bid.clone(),
        })
        .unwrap();
        ed.reload().unwrap();
        assert!(!ed.board().meta.archived);
    });
}
