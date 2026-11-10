use super::*;

#[test]
fn load_reattaches_orphan_card_and_reports() {
    with_temp_dir(|| {
        let (meta, _) = seed_board_with_card();
        let list_id = meta.lists[0].id.clone();
        // A card whose list_id names a list that does not exist.
        let mut orphan = Card::new("Lost".into());
        orphan.list_id = "ghostlst".into();
        card_store::save_card(&meta.id, &orphan).unwrap();

        let editor = BoardEditor::load(&meta.id).unwrap();
        assert_eq!(editor.diagnostics.len(), 1);
        // Reattached to the only (first) list, in memory and on disk.
        assert_eq!(editor.board().cards[&orphan.id].list_id, list_id);
        let reloaded = card_store::load_card(&meta.id, &orphan.id).unwrap();
        assert_eq!(reloaded.list_id, list_id);
        // And now visible in that list.
        assert!(editor.board().lists[0].card_ids.contains(&orphan.id));
    });
}

#[test]
fn load_strips_dangling_label_refs() {
    with_temp_dir(|| {
        let (meta, card_id) = seed_board_with_card();
        let mut card = card_store::load_card(&meta.id, &card_id).unwrap();
        card.label_ids = vec!["nolabel1".into(), "nolabel2".into()];
        card_store::save_card(&meta.id, &card).unwrap();

        let editor = BoardEditor::load(&meta.id).unwrap();
        assert_eq!(editor.diagnostics.len(), 1);
        assert!(editor.board().cards[&card_id].label_ids.is_empty());
        let reloaded = card_store::load_card(&meta.id, &card_id).unwrap();
        assert!(reloaded.label_ids.is_empty());
    });
}

#[test]
fn load_clean_board_has_no_diagnostics() {
    with_temp_dir(|| {
        let (meta, _) = seed_board_with_card();
        let editor = BoardEditor::load(&meta.id).unwrap();
        assert!(editor.diagnostics.is_empty());
    });
}

#[test]
fn apply_archive_card_persists_and_logs() {
    with_temp_dir(|| {
        let (meta, card_id) = seed_board_with_card();
        let mut editor = BoardEditor::load(&meta.id).unwrap();
        editor
            .apply(Command::ArchiveCard {
                card_id: card_id.clone(),
            })
            .unwrap();

        let card = editor.board().cards.get(&card_id).unwrap();
        assert!(card.archived);
        assert!(card.history.iter().any(|h| h.action == "Archived"));

        let reloaded = card_store::load_card(&meta.id, &card_id).unwrap();
        assert!(reloaded.archived);
        assert!(reloaded.history.iter().any(|h| h.action == "Archived"));
    });
}

#[test]
fn apply_restore_card_clears_archive_and_logs() {
    with_temp_dir(|| {
        let (meta, card_id) = seed_board_with_card();
        let mut editor = BoardEditor::load(&meta.id).unwrap();
        editor
            .apply(Command::ArchiveCard {
                card_id: card_id.clone(),
            })
            .unwrap();
        editor
            .apply(Command::RestoreCard {
                card_id: card_id.clone(),
            })
            .unwrap();

        let card = editor.board().cards.get(&card_id).unwrap();
        assert!(!card.archived);
        let actions: Vec<&str> = card.history.iter().map(|h| h.action.as_str()).collect();
        assert!(actions.contains(&"Archived"));
        assert!(actions.contains(&"Restored from archive"));
    });
}

/// Regression: a visible Card with archived (hidden) Cards interleaved
/// between it and its visible neighbor must still move one visible slot.
/// Before the fix, the move swapped raw `card_ids` slots and merely traded
/// places with a hidden Card — no visible change.
#[test]
fn move_up_skips_interleaved_archived_card() {
    with_temp_dir(|| {
        let mut meta = BoardMeta::new("Test".into());
        let lid = ListId::generate();
        meta.lists.push(ListMeta {
            id: lid.clone(),
            name: "TODO".into(),
            archived: false,
        });
        board_store::save_board(&meta).unwrap();

        // Visible "top" (pos -1), archived (0, 1), visible "bottom" (pos 2).
        let mk = |title: &str, pos: f64, archived: bool| {
            let mut c = Card::new(title.into());
            c.list_id = lid.clone();
            c.position = pos;
            c.archived = archived;
            card_store::save_card(&meta.id, &c).unwrap();
            c.id
        };
        let top = mk("top", -1.0, false);
        let _a1 = mk("arch1", 0.0, true);
        let _a2 = mk("arch2", 1.0, true);
        let bottom = mk("bottom", 2.0, false);

        let mut editor = BoardEditor::load(&meta.id).unwrap();
        // Visible order is [top, bottom]; bottom sits at raw index 3.
        let vis = editor.board().visible_cards(0, None);
        assert_eq!(vis.len(), 2);

        editor
            .apply(Command::MoveCard {
                card_id: bottom.clone(),
                direction: MoveDir::Up,
            })
            .unwrap();

        let visible_order = |ed: &BoardEditor| -> Vec<CardId> {
            ed.board()
                .visible_cards(0, None)
                .into_iter()
                .map(|i| ed.board().lists[0].card_ids[i].clone())
                .collect()
        };
        assert_eq!(
            visible_order(&editor),
            vec![bottom.clone(), top.clone()],
            "bottom moved above top"
        );
        // Selection follows the moved card.
        assert_eq!(editor.board().current_card_id(), Some(&bottom));

        editor.reload().unwrap();
        assert_eq!(
            visible_order(&editor),
            vec![bottom.clone(), top.clone()],
            "survives reload"
        );
    });
}

#[test]
fn move_card_all_directions_persist_across_reload() {
    with_temp_dir(|| {
        let mut meta = BoardMeta::new("Test".into());
        let la = ListId::generate();
        let lb = ListId::generate();
        meta.lists.push(ListMeta {
            id: la.clone(),
            name: "A".into(),
            archived: false,
        });
        meta.lists.push(ListMeta {
            id: lb.clone(),
            name: "B".into(),
            archived: false,
        });
        board_store::save_board(&meta).unwrap();
        let mut a = Vec::new();
        for (i, t) in ["a0", "a1", "a2"].iter().enumerate() {
            let mut c = Card::new((*t).into());
            c.list_id = la.clone();
            c.position = (i + 1) as f64;
            card_store::save_card(&meta.id, &c).unwrap();
            a.push(c.id);
        }
        let mut editor = BoardEditor::load(&meta.id).unwrap();
        assert_eq!(editor.board().lists[0].card_ids, a);

        // Down, then reload.
        editor
            .apply(Command::MoveCard {
                card_id: a[0].clone(),
                direction: MoveDir::Down,
            })
            .unwrap();
        let mem = editor.board().lists[0].card_ids.clone();
        assert_eq!(mem, vec![a[1].clone(), a[0].clone(), a[2].clone()]);
        editor.reload().unwrap();
        assert_eq!(
            editor.board().lists[0].card_ids,
            mem,
            "down survives reload"
        );

        // Up brings it back.
        editor
            .apply(Command::MoveCard {
                card_id: a[0].clone(),
                direction: MoveDir::Up,
            })
            .unwrap();
        editor.reload().unwrap();
        assert_eq!(editor.board().lists[0].card_ids, a, "up survives reload");

        // Right: a0 -> list B.
        editor
            .apply(Command::MoveCard {
                card_id: a[0].clone(),
                direction: MoveDir::Right,
            })
            .unwrap();
        editor.reload().unwrap();
        assert_eq!(
            editor.board().lists[0].card_ids,
            vec![a[1].clone(), a[2].clone()],
            "src after right"
        );
        assert_eq!(
            editor.board().lists[1].card_ids,
            vec![a[0].clone()],
            "dst after right"
        );
        assert_eq!(editor.board().cards[&a[0]].list_id, lb);

        // Left: a0 back to list A.
        editor
            .apply(Command::MoveCard {
                card_id: a[0].clone(),
                direction: MoveDir::Left,
            })
            .unwrap();
        editor.reload().unwrap();
        assert!(
            editor.board().lists[1].card_ids.is_empty(),
            "dst empty after left"
        );
        assert!(editor.board().lists[0].card_ids.contains(&a[0]));
    });
}
