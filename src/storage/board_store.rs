use std::fs;

use crate::model::board::BoardMeta;

use super::paths;
use super::{atomic_write, Result, StorageError};

pub fn ensure_base_dirs() -> Result<()> {
    fs::create_dir_all(paths::boards_dir())?;
    Ok(())
}

pub fn list_boards() -> Result<Vec<BoardMeta>> {
    let dir = paths::boards_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut boards = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let meta_path = entry.path().join("board.json");
            if meta_path.exists() {
                let data = fs::read_to_string(&meta_path)?;
                let meta: BoardMeta = serde_json::from_str(&data)?;
                boards.push(meta);
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
