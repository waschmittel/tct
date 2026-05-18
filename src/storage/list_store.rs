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

pub fn list_archived_lists(board_id: &str) -> Vec<CardList> {
    let dir = paths::board_dir(board_id);
    let mut lists = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("list-") && name.ends_with(".json") {
                if let Ok(data) = fs::read_to_string(entry.path()) {
                    if let Ok(list) = serde_json::from_str::<CardList>(&data) {
                        if list.archived {
                            lists.push(list);
                        }
                    }
                }
            }
        }
    }
    lists.sort_by(|a, b| a.name.cmp(&b.name));
    lists
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::board_store;
    use crate::test_support::with_temp_dir;

    #[test]
    fn save_load_roundtrip() {
        with_temp_dir(|| {
            let board = board_store::create_board("Board".into()).unwrap();
            let list = CardList::new("Backlog".into());
            save_list(&board.id, &list).unwrap();
            let loaded = load_list(&board.id, &list.id).unwrap();
            assert_eq!(loaded.name, "Backlog");
            assert!(loaded.card_ids.is_empty());
        });
    }

    #[test]
    fn load_missing_list_returns_not_found() {
        with_temp_dir(|| {
            let board = board_store::create_board("Board".into()).unwrap();
            let err = load_list(&board.id, "missing1").unwrap_err();
            assert!(matches!(err, StorageError::ListNotFound(_)));
        });
    }

    #[test]
    fn load_corrupt_list_returns_json_error() {
        with_temp_dir(|| {
            let board = board_store::create_board("Board".into()).unwrap();
            let path = paths::list_path(&board.id, "broken12");
            std::fs::write(&path, "not json").unwrap();
            let err = load_list(&board.id, "broken12").unwrap_err();
            assert!(matches!(err, StorageError::Json(_)));
        });
    }

    #[test]
    fn delete_list_file_removes_it() {
        with_temp_dir(|| {
            let board = board_store::create_board("Board".into()).unwrap();
            let list = CardList::new("Tmp".into());
            save_list(&board.id, &list).unwrap();
            assert!(paths::list_path(&board.id, &list.id).exists());
            delete_list_file(&board.id, &list.id).unwrap();
            assert!(!paths::list_path(&board.id, &list.id).exists());
        });
    }

    #[test]
    fn delete_missing_list_is_ok() {
        with_temp_dir(|| {
            let board = board_store::create_board("Board".into()).unwrap();
            delete_list_file(&board.id, "nope1234").unwrap();
        });
    }

    #[test]
    fn load_all_lists_preserves_order() {
        with_temp_dir(|| {
            let board = board_store::create_board("Board".into()).unwrap();
            let a = CardList::new("A".into());
            let b = CardList::new("B".into());
            let c = CardList::new("C".into());
            save_list(&board.id, &a).unwrap();
            save_list(&board.id, &b).unwrap();
            save_list(&board.id, &c).unwrap();
            // Request order: C, A, B
            let order = vec![c.id.clone(), a.id.clone(), b.id.clone()];
            let loaded = load_all_lists(&board.id, &order).unwrap();
            assert_eq!(loaded[0].name, "C");
            assert_eq!(loaded[1].name, "A");
            assert_eq!(loaded[2].name, "B");
        });
    }

    #[test]
    fn load_all_lists_errors_on_missing_id() {
        with_temp_dir(|| {
            let board = board_store::create_board("Board".into()).unwrap();
            let order = vec!["doesnotxx".to_string()];
            let err = load_all_lists(&board.id, &order).unwrap_err();
            assert!(matches!(err, StorageError::ListNotFound(_)));
        });
    }
}
