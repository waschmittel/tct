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

