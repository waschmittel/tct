use std::fs;

use crate::model::ids::ShortId;
use crate::model::list::CardList;

use super::paths;
use super::{atomic_write, Result, StorageError};

pub fn load_list(board_id: &str, list_id: &str) -> Result<CardList> {
    let path = paths::list_path(board_id, list_id);
    if !path.exists() {
        return Err(StorageError::ListNotFound(list_id.to_string()));
    }
    let data = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&data)?)
}

pub fn save_list(board_id: &str, list: &CardList) -> Result<()> {
    let json = serde_json::to_string_pretty(list)?;
    atomic_write(&paths::list_path(board_id, &list.id), json.as_bytes())?;
    Ok(())
}

pub fn delete_list_file(board_id: &str, list_id: &str) -> Result<()> {
    let path = paths::list_path(board_id, list_id);
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn load_all_lists(board_id: &str, list_order: &[ShortId]) -> Result<Vec<CardList>> {
    let mut lists = Vec::with_capacity(list_order.len());
    for id in list_order {
        lists.push(load_list(board_id, id)?);
    }
    Ok(lists)
}
