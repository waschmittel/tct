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
    Ok(serde_json::from_str(&data)?)
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

