use std::collections::HashMap;
use std::fs;

use crate::errln;
use crate::model::card::Card;
use crate::model::id::{BoardId, CardId};

use super::paths;
use super::{atomic_write, Result, StorageError};

/// The only place a `Card` is deserialized: `card_id` comes from the file name
/// and is stamped onto the result, because a Card's file name *is* its id
/// (ADR-0007). Whatever an `"id"` key in the file says is ignored.
pub fn load_card(board_id: &BoardId, card_id: &CardId) -> Result<Card> {
    let path = paths::card_path(board_id, card_id);
    if !path.exists() {
        return Err(StorageError::CardNotFound(card_id.to_string()));
    }
    let data = super::read_unlinked(&path)?;
    let mut val: serde_json::Value = serde_json::from_str(&data).map_err(|e| super::corrupt(&path, e))?;

    let mut migrated = false;

    // Migrate old "checklists" (Vec<{title, items}>) → flat "checklist" (Vec<ChecklistItem>)
    if let Some(checklists) = val.get("checklists")
        && checklists.is_array() {
            let mut flat_items = Vec::new();
            if let Some(arr) = checklists.as_array() {
                for cl in arr {
                    if let Some(items) = cl.get("items").and_then(|i| i.as_array()) {
                        for item in items {
                            flat_items.push(item.clone());
                        }
                    }
                }
            }
            val.as_object_mut().unwrap().remove("checklists");
            val.as_object_mut()
                .unwrap()
                .insert("checklist".to_string(), serde_json::Value::Array(flat_items));
            migrated = true;
        }

    // Migrate old "labels" (Vec<{name, color}>) → "label_ids" (Vec<CardId>)
    // Old labels without "id" are dropped here; they'll be re-created during board-level migration
    if let Some(labels) = val.get("labels")
        && labels.is_array() {
            val.as_object_mut().unwrap().remove("labels");
            if !val.as_object().unwrap().contains_key("label_ids") {
                val.as_object_mut().unwrap().insert(
                    "label_ids".to_string(),
                    serde_json::Value::Array(Vec::new()),
                );
            }
            migrated = true;
        }

    let mut card: Card = serde_json::from_value(val).map_err(|e| super::corrupt(&path, e))?;
    card.id = card_id.clone();

    if migrated {
        let _ = save_card(board_id, &card);
    }

    Ok(card)
}

pub fn save_card(board_id: &BoardId, card: &Card) -> Result<()> {
    // The id is the file name and is not in the file, so an unstamped card
    // would silently write itself to `card-.json`.
    debug_assert!(!card.id.is_empty(), "card id must be stamped before saving");
    let json = serde_json::to_string_pretty(card)?;
    atomic_write(&paths::card_path(board_id, &card.id), json.as_bytes())?;
    Ok(())
}

pub fn delete_card(board_id: &BoardId, card_id: &CardId) -> Result<()> {
    let path = paths::card_path(board_id, card_id);
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::board::BoardMeta;
    use crate::storage::board_store;
    use crate::test_support::with_temp_dir;

    fn make_board() -> BoardMeta {
        board_store::create_board("Board".into()).unwrap()
    }

    #[test]
    fn migration_old_checklists_format() {
        with_temp_dir(|| {
            let board = make_board();
            let card = Card::new("Task".into());
            // Write legacy JSON with "checklists" array format
            let legacy = serde_json::json!({
                "id": card.id,
                "title": card.title,
                "description": "",
                "label_ids": [],
                "due_date": null,
                "checklists": [
                    {
                        "title": "Checklist 1",
                        "items": [
                            { "text": "item A", "completed": false },
                            { "text": "item B", "completed": true }
                        ]
                    },
                    {
                        "title": "Checklist 2",
                        "items": [
                            { "text": "item C", "completed": false }
                        ]
                    }
                ],
                "archived": false,
                "created_at": card.created_at,
                "updated_at": card.updated_at
            });
            let path = paths::card_path(&board.id, &card.id);
            std::fs::write(&path, serde_json::to_string_pretty(&legacy).unwrap()).unwrap();

            let loaded = load_card(&board.id, &card.id).unwrap();
            assert_eq!(loaded.checklist.len(), 3);
            assert_eq!(loaded.checklist[0].text, "item A");
            assert!(!loaded.checklist[0].completed);
            assert_eq!(loaded.checklist[1].text, "item B");
            assert!(loaded.checklist[1].completed);
            assert_eq!(loaded.checklist[2].text, "item C");
        });
    }

    #[test]
    fn load_missing_card_returns_not_found() {
        with_temp_dir(|| {
            let board = make_board();
            let err = load_card(&board.id, &"doesnotxx".into()).unwrap_err();
            assert!(matches!(err, StorageError::CardNotFound(_)));
        });
    }

    #[test]
    fn load_corrupt_json_returns_error() {
        with_temp_dir(|| {
            let board = make_board();
            let path = paths::card_path(&board.id, &"bad12345".into());
            std::fs::write(&path, "{ not valid json").unwrap();
            let err = load_card(&board.id, &"bad12345".into()).unwrap_err();
            assert!(matches!(err, StorageError::Corrupt { .. }));
        });
    }

    #[test]
    fn load_all_cards_fails_on_corrupt_by_default() {
        with_temp_dir(|| {
            let board = make_board();
            let good = Card::new("Good".into());
            save_card(&board.id, &good).unwrap();
            std::fs::write(paths::card_path(&board.id, &"corrupt1".into()), "{ broken").unwrap();

            let err = load_all_cards(&board.id).unwrap_err();
            assert!(matches!(err, StorageError::Corrupt { .. }));
        });
    }

    #[test]
    fn load_all_cards_skips_corrupt_when_env_set() {
        with_temp_dir(|| {
            let board = make_board();
            let good = Card::new("Good".into());
            save_card(&board.id, &good).unwrap();
            std::fs::write(paths::card_path(&board.id, &"corrupt1".into()), "{ broken").unwrap();

            unsafe { std::env::set_var("TCT_SKIP_CORRUPT", "1") };
            let map = load_all_cards(&board.id);
            unsafe { std::env::remove_var("TCT_SKIP_CORRUPT") };

            let map = map.unwrap();
            assert_eq!(map.len(), 1);
            assert!(map.contains_key(&good.id));
        });
    }

    #[test]
    fn save_then_load_preserves_all_fields() {
        with_temp_dir(|| {
            let board = make_board();
            let mut card = Card::new("Roundtrip".into());
            card.description = "desc".into();
            card.due_date = Some(chrono::NaiveDate::from_ymd_opt(2030, 1, 15).unwrap());
            card.checklist.push(crate::model::card::ChecklistItem {
                text: "step".into(),
                completed: true,
            });
            save_card(&board.id, &card).unwrap();
            let loaded = load_card(&board.id, &card.id).unwrap();
            assert_eq!(loaded.title, card.title);
            assert_eq!(loaded.description, card.description);
            assert_eq!(loaded.due_date, card.due_date);
            assert_eq!(loaded.checklist.len(), 1);
            assert!(loaded.checklist[0].completed);
        });
    }

    #[test]
    fn delete_missing_card_is_ok() {
        with_temp_dir(|| {
            let board = make_board();
            // No card exists; delete should not error
            delete_card(&board.id, &"nonexist".into()).unwrap();
        });
    }

    #[test]
    fn save_overwrites_existing_card() {
        with_temp_dir(|| {
            let board = make_board();
            let mut card = Card::new("First".into());
            save_card(&board.id, &card).unwrap();
            card.title = "Second".into();
            save_card(&board.id, &card).unwrap();
            let loaded = load_card(&board.id, &card.id).unwrap();
            assert_eq!(loaded.title, "Second");
        });
    }

    #[test]
    fn list_archived_cards_filters() {
        with_temp_dir(|| {
            let board = make_board();
            let active = Card::new("Active".into());
            let mut archived = Card::new("Archived".into());
            archived.archived = true;
            save_card(&board.id, &active).unwrap();
            save_card(&board.id, &archived).unwrap();

            let list = list_archived_cards(&board.id).unwrap();
            assert_eq!(list.len(), 1);
            assert_eq!(list[0].title, "Archived");
        });
    }

    #[test]
    fn migration_old_labels_format() {
        with_temp_dir(|| {
            let board = make_board();
            let card = Card::new("Task".into());
            // Write legacy JSON with inline "labels" array (no ids)
            let legacy = serde_json::json!({
                "id": card.id,
                "title": card.title,
                "description": "",
                "due_date": null,
                "checklist": [],
                "labels": [
                    { "name": "bug", "color": "red" },
                    { "name": "urgent", "color": "orange" }
                ],
                "archived": false,
                "created_at": card.created_at,
                "updated_at": card.updated_at
            });
            let path = paths::card_path(&board.id, &card.id);
            std::fs::write(&path, serde_json::to_string_pretty(&legacy).unwrap()).unwrap();

            let loaded = load_card(&board.id, &card.id).unwrap();
            // Old labels without IDs are dropped; label_ids starts empty
            assert!(loaded.label_ids.is_empty());
        });
    }

    /// Writes `card-<file_id>.json` carrying a stray `"id"` key, as files
    /// written before ADR-0007 (and hand-edited ones) do.
    fn write_card_json(board: &BoardMeta, file_id: &str, declared_id: &str) {
        let card = Card::new("Task".into());
        let json = serde_json::json!({
            "id": declared_id,
            "list_id": "",
            "position": 1.0,
            "title": "Task",
            "description": "",
            "label_ids": [],
            "due_date": null,
            "checklist": [],
            "archived": false,
            "created_at": card.created_at,
            "updated_at": card.updated_at,
        });
        let path = paths::board_dir(&board.id).join(format!("card-{file_id}.json"));
        std::fs::write(&path, serde_json::to_string_pretty(&json).unwrap()).unwrap();
    }

    #[test]
    fn file_name_wins_over_a_declared_id() {
        with_temp_dir(|| {
            let board = make_board();
            write_card_json(&board, "aaaaaaaa", "bbbbbbbb");

            let card = load_card(&board.id, &"aaaaaaaa".into()).unwrap();
            assert_eq!(card.id, "aaaaaaaa");
        });
    }

    /// A symlinked card file would read from wherever it points, and its parse
    /// errors would quote that file's content back to the user.
    #[cfg(unix)]
    #[test]
    fn a_symlinked_card_file_is_refused() {
        with_temp_dir(|| {
            let board = make_board();
            let outside = paths::base_dir().join("outside.json");
            std::fs::write(&outside, "{}").unwrap();
            let link = paths::card_path(&board.id, &"aaaaaaaa".into());
            std::os::unix::fs::symlink(&outside, &link).unwrap();

            let err = load_card(&board.id, &"aaaaaaaa".into()).unwrap_err();
            assert!(matches!(err, StorageError::Symlink { .. }), "{err}");
        });
    }

    #[test]
    fn saved_cards_do_not_carry_an_id() {
        with_temp_dir(|| {
            let board = make_board();
            let card = Card::new("Task".into());
            save_card(&board.id, &card).unwrap();

            let raw = std::fs::read_to_string(paths::card_path(&board.id, &card.id)).unwrap();
            let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
            assert!(json.get("id").is_none(), "id should not be persisted: {raw}");
            assert_eq!(load_card(&board.id, &card.id).unwrap().id, card.id);
        });
    }

    #[test]
    fn files_whose_name_is_not_an_id_are_not_cards() {
        with_temp_dir(|| {
            let board = make_board();
            let good = Card::new("Good".into());
            save_card(&board.id, &good).unwrap();
            let dir = paths::board_dir(&board.id);
            std::fs::write(dir.join("card-not an id.json"), "{ irrelevant").unwrap();
            std::fs::write(dir.join("card-.DS_Store.json"), "{ irrelevant").unwrap();

            let cards = load_all_cards(&board.id).unwrap();
            assert_eq!(cards.len(), 1);
            assert!(cards.contains_key(&good.id));
        });
    }

}

/// The Card ids a board's directory holds, taken from the `card-<id>.json`
/// file names. A name whose id part is not a valid [`CardId`] was not written
/// by tct and is not a Card, so it is passed over.
fn card_ids_on_disk(board_id: &BoardId) -> Vec<CardId> {
    let mut ids = Vec::new();
    if let Ok(entries) = fs::read_dir(paths::board_dir(board_id)) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if let Some(rest) = name.strip_prefix("card-")
                && let Some(id) = rest.strip_suffix(".json")
                && let Ok(id) = CardId::parse(id)
            {
                ids.push(id);
            }
        }
    }
    ids
}

/// Read every Card of a board through [`load_card`] (so per-card migrations and
/// the id/file-name agreement check run), keeping those `keep` selects. An
/// unreadable card fails the whole load unless `TCT_SKIP_CORRUPT` is set, in
/// which case it is skipped with a warning.
fn read_cards(board_id: &BoardId, keep: impl Fn(&Card) -> bool) -> Result<Vec<Card>> {
    let mut cards = Vec::new();
    for id in card_ids_on_disk(board_id) {
        match load_card(board_id, &id) {
            Ok(card) if keep(&card) => cards.push(card),
            Ok(_) => {}
            Err(e) if super::skip_corrupt() => {
                errln!("tct: skipping {}: {e}", paths::card_path(board_id, &id).display());
            }
            Err(e) => return Err(e),
        }
    }
    Ok(cards)
}

/// Every Card stored for a board, keyed by id.
pub fn load_all_cards(board_id: &BoardId) -> Result<HashMap<CardId, Card>> {
    Ok(read_cards(board_id, |_| true)?
        .into_iter()
        .map(|card| (card.id.clone(), card))
        .collect())
}

/// Archived Cards of a board, newest first.
pub fn list_archived_cards(board_id: &BoardId) -> Result<Vec<Card>> {
    let mut cards = read_cards(board_id, |card| card.archived)?;
    cards.sort_by_key(|c| std::cmp::Reverse(c.updated_at));
    Ok(cards)
}
