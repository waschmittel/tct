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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::with_temp_dir;

    #[test]
    fn load_missing_board_returns_not_found() {
        with_temp_dir(|| {
            let err = load_board("nope1234").unwrap_err();
            assert!(matches!(err, StorageError::BoardNotFound(_)));
        });
    }

    #[test]
    fn list_boards_excludes_archived() {
        with_temp_dir(|| {
            let mut a = create_board("Active".into()).unwrap();
            let mut b = create_board("Archived".into()).unwrap();
            b.archived = true;
            save_board(&b).unwrap();
            // active list should not include b
            let active = list_boards().unwrap();
            assert!(active.iter().any(|x| x.name == "Active"));
            assert!(!active.iter().any(|x| x.name == "Archived"));
            // archived list should include b only
            let archived = list_archived_boards().unwrap();
            assert_eq!(archived.len(), 1);
            assert_eq!(archived[0].name, "Archived");
            // touch a to silence unused mut warning
            a.name = a.name.clone();
        });
    }

    #[test]
    fn board_order_roundtrip() {
        with_temp_dir(|| {
            let b1 = create_board("One".into()).unwrap();
            let b2 = create_board("Two".into()).unwrap();
            let order = vec![b2.id.clone(), b1.id.clone()];
            save_board_order(&order).unwrap();
            let loaded = load_board_order().unwrap();
            assert_eq!(loaded, order);
        });
    }

    #[test]
    fn list_boards_respects_order() {
        with_temp_dir(|| {
            let b1 = create_board("Alpha".into()).unwrap();
            let b2 = create_board("Beta".into()).unwrap();
            let b3 = create_board("Gamma".into()).unwrap();
            save_board_order(&[b3.id.clone(), b1.id.clone(), b2.id.clone()]).unwrap();
            let listed = list_boards().unwrap();
            assert_eq!(listed[0].name, "Gamma");
            assert_eq!(listed[1].name, "Alpha");
            assert_eq!(listed[2].name, "Beta");
        });
    }

    #[test]
    fn unordered_boards_alphabetical_fallback() {
        with_temp_dir(|| {
            // No saved order — fall back to alphabetical
            let _ = create_board("Charlie".into()).unwrap();
            let _ = create_board("Alpha".into()).unwrap();
            let _ = create_board("Bravo".into()).unwrap();
            let listed = list_boards().unwrap();
            assert_eq!(listed[0].name, "Alpha");
            assert_eq!(listed[1].name, "Bravo");
            assert_eq!(listed[2].name, "Charlie");
        });
    }

    #[test]
    fn append_to_order_no_duplicate() {
        with_temp_dir(|| {
            let b = create_board("X".into()).unwrap();
            append_to_order(&b.id).unwrap();
            append_to_order(&b.id).unwrap();
            let order = load_board_order().unwrap();
            assert_eq!(order.iter().filter(|id| **id == b.id).count(), 1);
        });
    }

    #[test]
    fn remove_from_order_works() {
        with_temp_dir(|| {
            let b1 = create_board("One".into()).unwrap();
            let b2 = create_board("Two".into()).unwrap();
            append_to_order(&b1.id).unwrap();
            append_to_order(&b2.id).unwrap();
            remove_from_order(&b1.id).unwrap();
            let order = load_board_order().unwrap();
            assert!(!order.contains(&b1.id));
            assert!(order.contains(&b2.id));
        });
    }

    #[test]
    fn delete_missing_board_is_ok() {
        with_temp_dir(|| {
            delete_board("doesnotxx").unwrap();
        });
    }

    #[test]
    fn save_creates_dir_and_meta() {
        with_temp_dir(|| {
            let meta = BoardMeta::new("New".into());
            save_board(&meta).unwrap();
            assert!(paths::board_dir(&meta.id).exists());
            assert!(paths::board_meta_path(&meta.id).exists());
        });
    }

    #[test]
    fn accent_color_persists() {
        with_temp_dir(|| {
            let mut meta = BoardMeta::new("Acc".into());
            meta.accent_color = crate::model::label::LabelColor::Purple;
            save_board(&meta).unwrap();
            let loaded = load_board(&meta.id).unwrap();
            assert_eq!(loaded.accent_color, crate::model::label::LabelColor::Purple);
        });
    }

    #[test]
    fn rename_board_persists() {
        with_temp_dir(|| {
            let mut meta = create_board("Old".into()).unwrap();
            meta.name = "New".into();
            save_board(&meta).unwrap();
            let loaded = load_board(&meta.id).unwrap();
            assert_eq!(loaded.name, "New");
        });
    }
}
