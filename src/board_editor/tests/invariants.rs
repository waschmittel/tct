use super::*;

#[test]
fn check_invariants_passes_on_well_formed_board() {
    let editor = invariant_board(|_| {});
    assert!(editor.check_invariants().is_ok());
}

#[test]
fn check_invariants_detects_orphan_card() {
    let editor = invariant_board(|b| {
        let id = b.lists[0].card_ids[0].clone();
        b.cards.get_mut(&id).unwrap().list_id = "ghostlst".into();
    });
    let err = editor.check_invariants().unwrap_err();
    assert!(err.contains("naming no known List"), "got: {err}");
}

#[test]
fn check_invariants_detects_selection_out_of_range() {
    let editor = invariant_board(|b| b.selected_card[0] = 5);
    let err = editor.check_invariants().unwrap_err();
    assert!(err.contains("ordinal"), "got: {err}");
}

#[test]
fn check_invariants_detects_diverged_card_ids() {
    let editor = invariant_board(|b| b.lists[0].card_ids.push("phantom0".into()));
    let err = editor.check_invariants().unwrap_err();
    assert!(err.contains("diverged"), "got: {err}");
}

#[test]
fn visible_cards_empty_for_no_search_match() {
    let board = loaded_board(vec![fixed_card("c1", "alpha")]);
    assert!(board.visible_cards(0, Some("zzz")).is_empty());
}

#[test]
fn visible_cards_skips_archived() {
    let mut cards = vec![fixed_card("id1", "a"), fixed_card("id2", "b")];
    cards[1].archived = true;
    let board = loaded_board(cards);
    assert_eq!(board.visible_cards(0, None), vec![0]);
}

#[test]
fn visible_cards_search_hides_non_matching() {
    let cards = vec![
        fixed_card("c1", "alpha"),
        fixed_card("c2", "BINGO"),
        fixed_card("c3", "gamma"),
    ];
    let board = loaded_board(cards);
    assert_eq!(board.visible_cards(0, Some("bingo")), vec![1]);
    assert_eq!(board.visible_cards(0, None), vec![0, 1, 2]);
}

#[test]
fn select_card_down_skips_to_next_search_match() {
    let cards = vec![
        fixed_card("c1", "alpha"),
        fixed_card("c2", "BINGO match"),
        fixed_card("c3", "gamma"),
    ];
    let mut editor = BoardEditor::from_loaded(loaded_board(cards));
    editor.select_card_down(Some("BINGO"));
    assert_eq!(editor.board().selected_card[0], 1);
    editor.select_card_down(Some("BINGO"));
    assert_eq!(editor.board().selected_card[0], 1);
}

#[test]
fn apply_missing_card_returns_not_found() {
    with_temp_dir(|| {
        let (meta, _) = seed_board_with_card();
        let mut editor = BoardEditor::load(&meta.id).unwrap();
        let bogus = CardId::generate();
        let err = editor
            .apply(Command::ArchiveCard { card_id: bogus })
            .unwrap_err();
        assert!(matches!(
            err,
            BoardEditorError::NotFound { kind: "card", .. }
        ));
    });
}

#[test]
fn focus_card_selects_correct_list_and_card() {
    let cards = vec![
        fixed_card("c1", "first"),
        fixed_card("c2", "second"),
        fixed_card("c3", "third"),
    ];
    let mut editor = BoardEditor::from_loaded(loaded_board(cards));
    assert_eq!(editor.board().current_card_id().unwrap(), "c1");

    assert!(editor.focus_card(&"c2".into(), None));
    assert_eq!(editor.board().selected_list, 0);
    assert_eq!(editor.board().current_card_id().unwrap(), "c2");

    // Non-existent card returns false and leaves current focus
    assert!(!editor.focus_card(&"c999".into(), None));
    assert_eq!(editor.board().current_card_id().unwrap(), "c2");
}

#[test]
fn focus_card_ignores_archived() {
    let mut c1 = fixed_card("c1", "first");
    c1.archived = true;
    let c2 = fixed_card("c2", "second");
    let mut editor = BoardEditor::from_loaded(loaded_board(vec![c1, c2]));
    // c1 is archived, so focusing c1 must fail
    assert!(!editor.focus_card(&"c1".into(), None));
}
