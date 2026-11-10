use super::*;

// ── Checklist reorder tests ───────────────────────────────────────────────

#[test]
fn checklist_reorder_basic() {
    let mut card = Card::new("Test".into());
    card.checklist = vec![
        ChecklistItem {
            text: "A".into(),
            completed: false,
        },
        ChecklistItem {
            text: "B".into(),
            completed: false,
        },
        ChecklistItem {
            text: "C".into(),
            completed: false,
        },
    ];

    // Swap A↔B (move index 0 down)
    card.checklist.swap(0, 1);
    assert_eq!(card.checklist[0].text, "B");
    assert_eq!(card.checklist[1].text, "A");
    assert_eq!(card.checklist[2].text, "C");
}

#[test]
fn checklist_reorder_move_up() {
    let mut card = Card::new("Test".into());
    card.checklist = vec![
        ChecklistItem {
            text: "A".into(),
            completed: true,
        },
        ChecklistItem {
            text: "B".into(),
            completed: false,
        },
    ];

    // Move B up: swap indices 1 and 0
    card.checklist.swap(1, 0);
    assert_eq!(card.checklist[0].text, "B");
    assert_eq!(card.checklist[1].text, "A");
    // Completion state should move with the item
    assert!(!card.checklist[0].completed);
    assert!(card.checklist[1].completed);
}

#[test]
fn checklist_reorder_at_top_is_noop() {
    let mut card = Card::new("Test".into());
    card.checklist = vec![
        ChecklistItem {
            text: "A".into(),
            completed: false,
        },
        ChecklistItem {
            text: "B".into(),
            completed: false,
        },
    ];
    let idx = 0usize;
    // Cannot move up from top (idx == 0)
    if idx > 0 {
        card.checklist.swap(idx, idx - 1);
    }
    assert_eq!(card.checklist[0].text, "A");
    assert_eq!(card.checklist[1].text, "B");
}

#[test]
fn checklist_reorder_at_bottom_is_noop() {
    let mut card = Card::new("Test".into());
    card.checklist = vec![
        ChecklistItem {
            text: "A".into(),
            completed: false,
        },
        ChecklistItem {
            text: "B".into(),
            completed: false,
        },
    ];
    let idx = 1usize;
    // Cannot move down from last
    if idx + 1 < card.checklist.len() {
        card.checklist.swap(idx, idx + 1);
    }
    assert_eq!(card.checklist[0].text, "A");
    assert_eq!(card.checklist[1].text, "B");
}

#[test]
fn checklist_reorder_empty_is_noop() {
    let mut card = Card::new("Test".into());
    let idx = 0usize;
    // No panic on empty checklist
    if idx + 1 < card.checklist.len() {
        card.checklist.swap(idx, idx + 1);
    }
    assert!(card.checklist.is_empty());
}

#[test]
fn checklist_reorder_persists() {
    with_temp_dir(|| {
        let board = board_store::create_board("Board".into()).unwrap();
        let mut card = Card::new("Task".into());
        card.checklist = vec![
            ChecklistItem {
                text: "First".into(),
                completed: false,
            },
            ChecklistItem {
                text: "Second".into(),
                completed: true,
            },
            ChecklistItem {
                text: "Third".into(),
                completed: false,
            },
        ];
        card_store::save_card(&board.id, &card).unwrap();

        // Reorder: move "Second" up (swap indices 1,0)
        card.checklist.swap(1, 0);
        card.touch();
        card_store::save_card(&board.id, &card).unwrap();

        let loaded = card_store::load_card(&board.id, &card.id).unwrap();
        assert_eq!(loaded.checklist[0].text, "Second");
        assert_eq!(loaded.checklist[1].text, "First");
        assert_eq!(loaded.checklist[2].text, "Third");
        assert!(loaded.checklist[0].completed);
        assert!(!loaded.checklist[1].completed);
    });
}

// ── Due date deletion tests ───────────────────────────────────────────────

#[test]
fn due_date_set_and_clear() {
    with_temp_dir(|| {
        let board = board_store::create_board("Board".into()).unwrap();
        let mut card = Card::new("Task".into());
        let date = chrono::NaiveDate::from_ymd_opt(2099, 12, 31).unwrap();
        card.due_date = Some(date);
        card_store::save_card(&board.id, &card).unwrap();

        let loaded = card_store::load_card(&board.id, &card.id).unwrap();
        assert_eq!(loaded.due_date, Some(date));

        // Clear due date
        let mut updated = loaded;
        updated.due_date = None;
        updated.touch();
        card_store::save_card(&board.id, &updated).unwrap();

        let final_card = card_store::load_card(&board.id, &updated.id).unwrap();
        assert!(final_card.due_date.is_none());
    });
}

#[test]
fn due_date_clear_when_none_is_noop() {
    let mut card = Card::new("Task".into());
    assert!(card.due_date.is_none());
    // Clearing already-None due date should not panic
    card.due_date = None;
    card.touch();
    assert!(card.due_date.is_none());
}

#[test]
fn due_date_cli_set_and_clear_via_none() {
    with_temp_dir(|| {
        let mut board = board_store::create_board("Board".into()).unwrap();
        let list = crate::test_support::add_list(&mut board, "To Do");
        let mut card = Card::new("Deadline task".into());
        crate::test_support::put_cards(&board.id, &list, std::slice::from_mut(&mut card));

        // Set due date via CLI
        crate::cli::run(&[
            "cards".to_string(),
            "Board".to_string(),
            "--edit".to_string(),
            "Deadline".to_string(),
            "--due".to_string(),
            "2099-12-31".to_string(),
        ])
        .unwrap();

        let loaded = card_store::load_card(&board.id, &card.id).unwrap();
        assert!(loaded.due_date.is_some());

        // Clear via CLI --due none
        crate::cli::run(&[
            "cards".to_string(),
            "Board".to_string(),
            "--edit".to_string(),
            "Deadline".to_string(),
            "--due".to_string(),
            "none".to_string(),
        ])
        .unwrap();

        let cleared = card_store::load_card(&board.id, &card.id).unwrap();
        assert!(cleared.due_date.is_none());
    });
}

// ── Label reorder tests ───────────────────────────────────────────────────

#[test]
fn label_reorder_basic() {
    let mut board = crate::model::board::BoardMeta::new("Board".into());
    board.labels = vec![
        Label::new("alpha".into(), LabelColor::Red),
        Label::new("beta".into(), LabelColor::Green),
        Label::new("gamma".into(), LabelColor::Blue),
    ];
    // Move "beta" down (swap 1 and 2)
    board.labels.swap(1, 2);
    assert_eq!(board.labels[0].name, "alpha");
    assert_eq!(board.labels[1].name, "gamma");
    assert_eq!(board.labels[2].name, "beta");
}

#[test]
fn label_reorder_persists() {
    with_temp_dir(|| {
        let mut board = board_store::create_board("Board".into()).unwrap();
        board.labels = vec![
            Label::new("first".into(), LabelColor::Red),
            Label::new("second".into(), LabelColor::Green),
            Label::new("third".into(), LabelColor::Blue),
        ];
        board_store::save_board(&board).unwrap();

        // Swap "first" and "second"
        board.labels.swap(0, 1);
        board_store::save_board(&board).unwrap();

        let loaded = board_store::load_board(&board.id).unwrap();
        assert_eq!(loaded.labels[0].name, "second");
        assert_eq!(loaded.labels[1].name, "first");
        assert_eq!(loaded.labels[2].name, "third");
    });
}

#[test]
fn label_reorder_move_last_to_first() {
    with_temp_dir(|| {
        let mut board = board_store::create_board("Board".into()).unwrap();
        board.labels = vec![
            Label::new("a".into(), LabelColor::Red),
            Label::new("b".into(), LabelColor::Green),
            Label::new("c".into(), LabelColor::Blue),
        ];
        // Move "c" up twice: c,b then c,a  (bubble-up style)
        board.labels.swap(2, 1);
        board.labels.swap(1, 0);
        board_store::save_board(&board).unwrap();

        let loaded = board_store::load_board(&board.id).unwrap();
        assert_eq!(loaded.labels[0].name, "c");
        assert_eq!(loaded.labels[1].name, "a");
        assert_eq!(loaded.labels[2].name, "b");
    });
}

#[test]
fn label_reorder_at_top_is_noop() {
    let mut board = crate::model::board::BoardMeta::new("Board".into());
    board.labels = vec![
        Label::new("only".into(), LabelColor::Red),
        Label::new("two".into(), LabelColor::Green),
    ];
    let idx = 0usize;
    // Attempting to move up from index 0: idx > 0 is false, no swap
    if idx > 0 {
        board.labels.swap(idx, idx - 1);
    }
    assert_eq!(board.labels[0].name, "only");
    assert_eq!(board.labels[1].name, "two");
}

#[test]
fn label_reorder_at_bottom_is_noop() {
    let mut board = crate::model::board::BoardMeta::new("Board".into());
    board.labels = vec![
        Label::new("one".into(), LabelColor::Red),
        Label::new("two".into(), LabelColor::Green),
    ];
    let idx = 1usize;
    // Attempting to move down from last index: no swap
    if idx + 1 < board.labels.len() {
        board.labels.swap(idx, idx + 1);
    }
    assert_eq!(board.labels[0].name, "one");
    assert_eq!(board.labels[1].name, "two");
}

#[test]
fn label_reorder_preserves_ids() {
    let mut board = crate::model::board::BoardMeta::new("Board".into());
    let l1 = Label::new("alpha".into(), LabelColor::Red);
    let l2 = Label::new("beta".into(), LabelColor::Green);
    let id1 = l1.id.clone();
    let id2 = l2.id.clone();
    board.labels = vec![l1, l2];
    board.labels.swap(0, 1);
    // IDs travel with their labels
    assert_eq!(board.labels[0].id, id2);
    assert_eq!(board.labels[1].id, id1);
}

#[test]
fn move_card_between_lists_lands_at_same_visible_slot() {
    with_temp_dir(|| {
        let (mut ed, a, b) = two_list_editor(&["a0", "a1", "a2"], &["b0"]);
        // Move a1 (visible slot 1) right into list B.
        ed.apply(Command::MoveCard {
            card_id: a[1].clone(),
            direction: MoveDir::Right,
        })
        .unwrap();
        assert_eq!(
            ed.board().lists[0].card_ids,
            vec![a[0].clone(), a[2].clone()]
        );
        assert_eq!(
            ed.board().lists[1].card_ids,
            vec![b[0].clone(), a[1].clone()]
        );
        assert_eq!(ed.board().cards[&a[1]].list_id, ed.board().lists[1].id);
        // Persisted.
        ed.reload().unwrap();
        assert_eq!(
            ed.board().lists[1].card_ids,
            vec![b[0].clone(), a[1].clone()]
        );
    });
}

#[test]
fn move_card_clamps_to_end_when_dst_shorter() {
    with_temp_dir(|| {
        let (mut ed, a, b) = two_list_editor(&["a0", "a1", "a2", "a3", "a4"], &["b0", "b1"]);
        // a4 is at visible slot 4; list B has only 2 slots → lands at end.
        ed.apply(Command::MoveCard {
            card_id: a[4].clone(),
            direction: MoveDir::Right,
        })
        .unwrap();
        assert_eq!(
            ed.board().lists[1].card_ids,
            vec![b[0].clone(), b[1].clone(), a[4].clone()]
        );
        ed.reload().unwrap();
        assert_eq!(
            ed.board().lists[1].card_ids,
            vec![b[0].clone(), b[1].clone(), a[4].clone()]
        );
    });
}
