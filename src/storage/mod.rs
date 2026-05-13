pub mod board_store;
pub mod card_store;
pub mod list_store;
pub mod paths;

use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Board not found: {0}")]
    BoardNotFound(String),
    #[error("List not found: {0}")]
    ListNotFound(String),
    #[error("Card not found: {0}")]
    CardNotFound(String),
}

pub type Result<T> = std::result::Result<T, StorageError>;

fn atomic_write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, contents)?;
    fs::rename(&tmp, path)?;
    Ok(())
}
