use super::*;

#[test]
fn version_is_stamped() {
    // build.rs always sets a non-empty TCT_VERSION (tag or snap-…).
    assert!(!crate::VERSION.is_empty());
}

#[test]
fn board_roundtrip() {
    with_temp_dir(|| {
        let board = board_store::create_board("My Board".into()).unwrap();
        let loaded = board_store::load_board(&board.id).unwrap();
        assert_eq!(loaded.name, "My Board");
        assert!(loaded.lists.is_empty());

        let boards = board_store::list_boards().unwrap();
        assert_eq!(boards.len(), 1);
        assert_eq!(boards[0].name, "My Board");
    });
}

#[test]
fn board_delete() {
    with_temp_dir(|| {
        let board = board_store::create_board("Delete Me".into()).unwrap();
        board_store::delete_board(&board.id).unwrap();
        let boards = board_store::list_boards().unwrap();
        assert!(boards.is_empty());
    });
}

#[test]
fn lists_persist_inline_in_the_board() {
    with_temp_dir(|| {
        let mut board = board_store::create_board("Board".into()).unwrap();
        let list_id = crate::test_support::add_list(&mut board, "To Do");

        let loaded = board_store::load_board(&board.id).unwrap();

        assert_eq!(loaded.lists.len(), 1);
        assert_eq!(loaded.lists[0].id, list_id);
        assert_eq!(loaded.lists[0].name, "To Do");
    });
}

#[test]
fn card_roundtrip() {
    with_temp_dir(|| {
        let board = board_store::create_board("Board".into()).unwrap();
        let label = Label::new("BUG".into(), LabelColor::Red);
        let label_id = label.id.clone();
        let mut card = Card::new("Fix bug".into());
        card.description = "Important fix".into();
        card.label_ids.push(label_id.clone());
        card.checklist.push(ChecklistItem {
            text: "Reproduce".into(),
            completed: true,
        });
        card.checklist.push(ChecklistItem {
            text: "Fix".into(),
            completed: false,
        });

        card_store::save_card(&board.id, &card).unwrap();
        let loaded = card_store::load_card(&board.id, &card.id).unwrap();
        assert_eq!(loaded.title, "Fix bug");
        assert_eq!(loaded.description, "Important fix");
        assert_eq!(loaded.label_ids.len(), 1);
        assert_eq!(loaded.label_ids[0], label_id);
        assert_eq!(loaded.checklist.len(), 2);
        assert!(loaded.checklist[0].completed);
    });
}

/// Moving a Card between Lists is one Card write; membership is derived
/// from `list_id`, never stored on the List (ADR-0006).
#[test]
fn full_lifecycle() {
    with_temp_dir(|| {
        let mut board = board_store::create_board("Project".into()).unwrap();
        let todo = crate::test_support::add_list(&mut board, "To Do");
        let done = crate::test_support::add_list(&mut board, "Done");
        let mut cards = vec![Card::new("Task 1".into()), Card::new("Task 2".into())];
        crate::test_support::put_cards(&board.id, &todo, &mut cards);

        cards[1].list_id = done.clone();
        cards[1].position = 1.0;
        card_store::save_card(&board.id, &cards[1]).unwrap();

        let stored = card_store::load_all_cards(&board.id).unwrap();
        let lists = crate::model::list::build_lists(&board, &stored, false);
        assert_eq!(lists[0].card_ids, vec![cards[0].id.clone()]);
        assert_eq!(lists[1].card_ids, vec![cards[1].id.clone()]);
    });
}
