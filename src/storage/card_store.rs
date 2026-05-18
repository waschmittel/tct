use std::fs;

use crate::model::card::Card;

use super::paths;
use super::{atomic_write, Result, StorageError};

pub fn load_card(board_id: &str, card_id: &str) -> Result<Card> {
    let path = paths::card_path(board_id, card_id);
    if !path.exists() {
        return Err(StorageError::CardNotFound(card_id.to_string()));
    }
    let data = fs::read_to_string(&path)?;
    let mut val: serde_json::Value = serde_json::from_str(&data)?;

    let mut migrated = false;

    // Migrate old "checklists" (Vec<{title, items}>) → flat "checklist" (Vec<ChecklistItem>)
    if let Some(checklists) = val.get("checklists") {
        if checklists.is_array() {
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
    }

    // Migrate old "labels" (Vec<{name, color}>) → "label_ids" (Vec<ShortId>)
    // Old labels without "id" are dropped here; they'll be re-created during board-level migration
    if let Some(labels) = val.get("labels") {
        if labels.is_array() {
            val.as_object_mut().unwrap().remove("labels");
            if !val.as_object().unwrap().contains_key("label_ids") {
                val.as_object_mut().unwrap().insert(
                    "label_ids".to_string(),
                    serde_json::Value::Array(Vec::new()),
                );
            }
            migrated = true;
        }
    }

    let card: Card = serde_json::from_value(val)?;

    if migrated {
        let _ = save_card(board_id, &card);
    }

    Ok(card)
}

pub fn save_card(board_id: &str, card: &Card) -> Result<()> {
    let json = serde_json::to_string_pretty(card)?;
    atomic_write(&paths::card_path(board_id, &card.id), json.as_bytes())?;
    Ok(())
}

pub fn delete_card(board_id: &str, card_id: &str) -> Result<()> {
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
    use std::env;

    fn with_temp_dir<F: FnOnce()>(f: F) {
        let dir = tempfile::tempdir().unwrap();
        unsafe { env::set_var("TCT_DATA_DIR", dir.path()) };
        board_store::ensure_base_dirs().unwrap();
        f();
        unsafe { env::remove_var("TCT_DATA_DIR") };
    }

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
}

pub fn list_archived_cards(board_id: &str) -> Vec<Card> {
    let dir = paths::board_dir(board_id);
    let mut cards = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("card-") && name.ends_with(".json") {
                if let Ok(data) = fs::read_to_string(entry.path()) {
                    if let Ok(card) = serde_json::from_str::<Card>(&data) {
                        if card.archived {
                            cards.push(card);
                        }
                    }
                }
            }
        }
    }
    cards.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    cards
}
