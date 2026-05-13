use std::fs;

use crate::model::board::BoardMeta;
use crate::model::ids::ShortId;

use super::paths;
use super::{atomic_write, Result, StorageError};

pub fn ensure_base_dirs() -> Result<()> {
    fs::create_dir_all(paths::boards_dir())?;
    Ok(())
}

pub fn load_board_order() -> Result<Vec<ShortId>> {
    let path = paths::board_order_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&data)?)
}

pub fn save_board_order(order: &[ShortId]) -> Result<()> {
    let json = serde_json::to_string_pretty(order)?;
    atomic_write(&paths::board_order_path(), json.as_bytes())?;
    Ok(())
}

pub fn list_boards() -> Result<Vec<BoardMeta>> {
    let dir = paths::boards_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut all_boards = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let meta_path = entry.path().join("board.json");
            if meta_path.exists() {
                let data = fs::read_to_string(&meta_path)?;
                let meta: BoardMeta = serde_json::from_str(&data)?;
                if !meta.archived {
                    all_boards.push(meta);
                }
            }
        }
    }

    let order = load_board_order().unwrap_or_default();
    let mut ordered: Vec<BoardMeta> = Vec::new();
    let mut remaining: Vec<BoardMeta> = all_boards;

    for id in &order {
        if let Some(pos) = remaining.iter().position(|b| &b.id == id) {
            ordered.push(remaining.remove(pos));
        }
    }
    remaining.sort_by(|a, b| a.name.cmp(&b.name));
    ordered.extend(remaining);

    Ok(ordered)
}

pub fn list_archived_boards() -> Result<Vec<BoardMeta>> {
    let dir = paths::boards_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut boards = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let meta_path = entry.path().join("board.json");
            if meta_path.exists() {
                let data = fs::read_to_string(&meta_path)?;
                let meta: BoardMeta = serde_json::from_str(&data)?;
                if meta.archived {
                    boards.push(meta);
                }
            }
        }
    }
    boards.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(boards)
}

pub fn load_board(board_id: &str) -> Result<BoardMeta> {
    let path = paths::board_meta_path(board_id);
    if !path.exists() {
        return Err(StorageError::BoardNotFound(board_id.to_string()));
    }
    let data = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&data)?)
}

pub fn save_board(meta: &BoardMeta) -> Result<()> {
    let dir = paths::board_dir(&meta.id);
    fs::create_dir_all(&dir)?;
    let json = serde_json::to_string_pretty(meta)?;
    atomic_write(&paths::board_meta_path(&meta.id), json.as_bytes())?;
    Ok(())
}

pub fn append_to_order(board_id: &ShortId) -> Result<()> {
    let mut order = load_board_order().unwrap_or_default();
    if !order.contains(board_id) {
        order.push(board_id.clone());
        save_board_order(&order)?;
    }
    Ok(())
}

pub fn remove_from_order(board_id: &ShortId) -> Result<()> {
    let mut order = load_board_order().unwrap_or_default();
    order.retain(|id| id != board_id);
    save_board_order(&order)?;
    Ok(())
}

#[cfg(test)]
pub fn create_board(name: String) -> Result<BoardMeta> {
    let meta = BoardMeta::new(name);
    save_board(&meta)?;
    Ok(meta)
}

pub fn delete_board(board_id: &str) -> Result<()> {
    let dir = paths::board_dir(board_id);
    if dir.exists() {
        fs::remove_dir_all(dir)?;
    }
    Ok(())
}
