//! One-time, on-load migration from the legacy per-List storage format to the
//! card-owned membership format.
//!
//! Legacy: each List lived in its own `list-<id>.json` carrying a `card_ids`
//! array, and `board.json` held an ordered `list_order`. Membership and a
//! Card's archived flag thus lived in two different files, which could diverge
//! (see ADR / orphan-card hazard).
//!
//! New: `board.json` carries an ordered `lists: [ListMeta]`, and each Card
//! stores its own `list_id` + fractional `position`. There are no list files.
//!
//! [`migrate_if_needed`] is idempotent and cheap once migrated (it returns as
//! soon as it finds no `list-*.json` files), so it is safe to call on every
//! board load.

use std::fs;
use std::path::PathBuf;

use crate::model::board::{BoardMeta, ListMeta};
use crate::model::list::CardList;

use super::{card_store, paths, Result};

fn list_files(board_id: &str) -> Vec<PathBuf> {
    let dir = paths::board_dir(board_id);
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("list-") && name.ends_with(".json") {
                files.push(entry.path());
            }
        }
    }
    files
}

/// Migrate a board in place if it is still in the legacy format. No-op when no
/// `list-*.json` files exist.
pub fn migrate_if_needed(board_id: &str) -> Result<()> {
    let files = list_files(board_id);
    if files.is_empty() {
        return Ok(());
    }

    // Read board.json directly (not via board_store::load_board — that calls us).
    let meta_path = paths::board_meta_path(board_id);
    let data = fs::read_to_string(&meta_path)?;
    let mut meta: BoardMeta = serde_json::from_str(&data)?;

    // Parse every legacy list file into the (still-compatible) CardList shape.
    let mut lists: Vec<CardList> = Vec::new();
    for path in &files {
        if let Ok(raw) = fs::read_to_string(path)
            && let Ok(list) = serde_json::from_str::<CardList>(&raw)
        {
            lists.push(list);
        }
    }

    // Stamp list_id + 1-based position onto each Card and rewrite it.
    for list in &lists {
        for (i, card_id) in list.card_ids.iter().enumerate() {
            if let Ok(mut card) = card_store::load_card(board_id, card_id) {
                card.list_id = list.id.clone();
                card.position = (i + 1) as f64;
                let _ = card_store::save_card(board_id, &card);
            }
        }
    }

    // Build ordered ListMeta: active Lists in legacy list_order first, then any
    // remaining active Lists, then archived Lists.
    let mut metas: Vec<ListMeta> = Vec::new();
    let push_meta = |l: &CardList, metas: &mut Vec<ListMeta>| {
        if !metas.iter().any(|m| m.id == l.id) {
            metas.push(ListMeta { id: l.id.clone(), name: l.name.clone(), archived: l.archived });
        }
    };
    for id in &meta.list_order {
        if let Some(l) = lists.iter().find(|l| &l.id == id && !l.archived) {
            push_meta(l, &mut metas);
        }
    }
    for l in lists.iter().filter(|l| !l.archived) {
        push_meta(l, &mut metas);
    }
    for l in lists.iter().filter(|l| l.archived) {
        push_meta(l, &mut metas);
    }

    meta.lists = metas;
    meta.list_order.clear();
    super::board_store::save_board(&meta)?;

    // Drop the now-redundant list files.
    for path in &files {
        let _ = fs::remove_file(path);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::card::Card;
    use crate::storage::{board_store, list_store};
    use crate::test_support::with_temp_dir;

    #[test]
    fn migrates_legacy_board_to_card_owned() {
        with_temp_dir(|| {
            // Build a legacy board by hand: board.json with list_order + list files.
            let mut meta = BoardMeta::new("Legacy".into());
            let mut backlog = CardList::new("Backlog".into());
            let mut done = CardList::new("Done".into());

            let mut c1 = Card::new("First".into());
            let mut c2 = Card::new("Second".into());
            let mut c3 = Card::new("Third".into());
            backlog.card_ids = vec![c1.id.clone(), c2.id.clone()];
            done.card_ids = vec![c3.id.clone()];

            // Save the board first so its directory exists for the cards/lists.
            meta.lists.clear();
            meta.list_order = vec![backlog.id.clone(), done.id.clone()];
            board_store::save_board(&meta).unwrap();

            // Legacy: cards have no list_id/position; save raw.
            for c in [&mut c1, &mut c2, &mut c3] {
                c.list_id = String::new();
                c.position = 0.0;
                card_store::save_card(&meta.id, c).unwrap();
            }
            list_store::save_list(&meta.id, &backlog).unwrap();
            list_store::save_list(&meta.id, &done).unwrap();

            migrate_if_needed(&meta.id).unwrap();

            // board.json now has ordered ListMeta, no list_order.
            let migrated = board_store::load_board(&meta.id).unwrap();
            assert_eq!(migrated.lists.len(), 2);
            assert_eq!(migrated.lists[0].name, "Backlog");
            assert_eq!(migrated.lists[1].name, "Done");
            assert!(migrated.list_order.is_empty());

            // Cards carry list_id + ascending position.
            let cards = card_store::load_all_cards(&meta.id).unwrap();
            let g1 = &cards[&c1.id];
            let g2 = &cards[&c2.id];
            let g3 = &cards[&c3.id];
            assert_eq!(g1.list_id, backlog.id);
            assert_eq!(g2.list_id, backlog.id);
            assert_eq!(g3.list_id, done.id);
            assert!(g1.position < g2.position);

            // List files are gone; re-running is a no-op.
            assert!(list_files(&meta.id).is_empty());
            migrate_if_needed(&meta.id).unwrap();
        });
    }
}
