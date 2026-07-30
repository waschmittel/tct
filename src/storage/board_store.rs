use std::fs;

use crate::errln;
use crate::model::board::BoardMeta;
use crate::model::id::BoardId;

use super::paths;
use super::{atomic_write, Result, StorageError};

pub fn ensure_base_dirs() -> Result<()> {
    fs::create_dir_all(paths::boards_dir())?;
    Ok(())
}

pub fn load_board_order() -> Result<Vec<BoardId>> {
    let path = paths::board_order_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data = super::read_unlinked(&path)?;
    serde_json::from_str(&data).map_err(|e| super::corrupt(&path, e))
}

pub fn save_board_order(order: &[BoardId]) -> Result<()> {
    let json = serde_json::to_string_pretty(order)?;
    atomic_write(&paths::board_order_path(), json.as_bytes())?;
    Ok(())
}

/// Read every `<board id>/board.json` under the boards directory. Entries that
/// are not a real directory named by a valid [`BoardId`] are not boards and are
/// passed over — including symlinks, since `DirEntry::file_type` reports the
/// link itself rather than what it points at.
fn read_all_boards() -> Result<Vec<BoardMeta>> {
    let dir = paths::boards_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }
    super::reject_symlink(&dir)?;

    let mut boards = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let Ok(board_id) = BoardId::parse(&entry.file_name().to_string_lossy()) else {
            continue;
        };
        let meta_path = paths::board_meta_path(&board_id);
        if !meta_path.exists() {
            continue;
        }
        match read_board_at(&board_id) {
            Ok(meta) => boards.push(meta),
            Err(e) if super::skip_corrupt() => {
                errln!("tct: skipping {}: {e}", meta_path.display());
            }
            Err(e) => return Err(e),
        }
    }
    Ok(boards)
}

/// The only place a `BoardMeta` is deserialized: `board_id` comes from the
/// directory name and is stamped onto the result, because a Board's directory
/// *is* its id (ADR-0007). Whatever an `"id"` key in the file says is ignored.
///
/// Bypasses [`load_board`] — the migration reads `board.json` too, and going
/// through `load_board` would recurse back into it.
pub(super) fn read_board_at(board_id: &BoardId) -> Result<BoardMeta> {
    let path = paths::board_meta_path(board_id);
    let data = super::read_unlinked(&path)?;
    let mut meta: BoardMeta = serde_json::from_str(&data).map_err(|e| super::corrupt(&path, e))?;
    meta.id = board_id.clone();
    Ok(meta)
}

pub fn list_boards() -> Result<Vec<BoardMeta>> {
    let all_boards: Vec<BoardMeta> = read_all_boards()?
        .into_iter()
        .filter(|meta| !meta.archived)
        .collect();

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
    let mut boards: Vec<BoardMeta> = read_all_boards()?
        .into_iter()
        .filter(|meta| meta.archived)
        .collect();
    boards.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(boards)
}

pub fn load_board(board_id: &BoardId) -> Result<BoardMeta> {
    let path = paths::board_meta_path(board_id);
    if !path.exists() {
        return Err(StorageError::BoardNotFound(board_id.to_string()));
    }
    super::migrate::migrate_if_needed(board_id)?;
    read_board_at(board_id)
}

pub fn save_board(meta: &BoardMeta) -> Result<()> {
    // The id is the directory name and is not in the file, so an unstamped
    // board would silently write itself over the boards directory itself.
    debug_assert!(!meta.id.is_empty(), "board id must be stamped before saving");
    let dir = paths::board_dir(&meta.id);
    fs::create_dir_all(&dir)?;
    let json = serde_json::to_string_pretty(meta)?;
    atomic_write(&paths::board_meta_path(&meta.id), json.as_bytes())?;
    Ok(())
}

pub fn append_to_order(board_id: &BoardId) -> Result<()> {
    let mut order = load_board_order().unwrap_or_default();
    if !order.contains(board_id) {
        order.push(board_id.clone());
        save_board_order(&order)?;
    }
    Ok(())
}

pub fn remove_from_order(board_id: &BoardId) -> Result<()> {
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

pub fn delete_board(board_id: &BoardId) -> Result<()> {
    // `board_dir("")` is the boards directory itself; deleting it recursively
    // would take every board with it.
    if board_id.is_empty() {
        return Err(StorageError::BoardNotFound(board_id.to_string()));
    }
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
            let err = load_board(&"nope1234".into()).unwrap_err();
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
    fn delete_board_refuses_the_empty_id() {
        with_temp_dir(|| {
            let real = create_board("Keep".into()).unwrap();

            let err = delete_board(&BoardId::default()).unwrap_err();

            assert!(matches!(err, StorageError::BoardNotFound(_)), "{err}");
            assert!(paths::board_dir(&real.id).exists(), "other boards must survive");
        });
    }

    #[test]
    fn delete_missing_board_is_ok() {
        with_temp_dir(|| {
            delete_board(&"doesnotxx".into()).unwrap();
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

    /// Writes `boards/<dir>/board.json` carrying a stray `"id"` key, as files
    /// written before ADR-0007 (and hand-edited ones) do.
    fn write_board_json(dir: &str, declared_id: &str) {
        let meta = BoardMeta::new("Planted".into());
        let json = serde_json::json!({
            "id": declared_id,
            "name": "Planted",
            "description": "",
            "lists": [],
            "labels": [],
            "archived": true,
            "created_at": meta.created_at,
            "updated_at": meta.updated_at,
        });
        let board_dir = paths::boards_dir().join(dir);
        fs::create_dir_all(&board_dir).unwrap();
        fs::write(board_dir.join("board.json"), serde_json::to_string_pretty(&json).unwrap())
            .unwrap();
    }

    #[test]
    fn directory_name_wins_over_a_declared_id() {
        with_temp_dir(|| {
            write_board_json("planted1", "otherbrd");

            let listed = list_archived_boards().unwrap();
            assert_eq!(listed.len(), 1);
            assert_eq!(listed[0].id, "planted1");
        });
    }

    /// Validated ids keep a *path* inside the data dir; a symlink redirects in
    /// the filesystem instead, where the path cannot show it.
    #[cfg(unix)]
    #[test]
    fn a_symlinked_board_directory_is_not_a_board() {
        with_temp_dir(|| {
            let real = create_board("Real".into()).unwrap();
            let outside = paths::base_dir().join("elsewhere");
            fs::create_dir_all(&outside).unwrap();
            fs::write(outside.join("board.json"), "{\"name\":\"Planted\"}").unwrap();
            std::os::unix::fs::symlink(&outside, paths::boards_dir().join("planted1")).unwrap();

            let listed = list_boards().unwrap();

            assert_eq!(listed.len(), 1);
            assert_eq!(listed[0].id, real.id);
        });
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_boards_directory_is_refused() {
        with_temp_dir(|| {
            let outside = paths::base_dir().join("elsewhere");
            fs::create_dir_all(&outside).unwrap();
            fs::remove_dir_all(paths::boards_dir()).ok();
            std::os::unix::fs::symlink(&outside, paths::boards_dir()).unwrap();

            let err = list_boards().unwrap_err();
            assert!(matches!(err, StorageError::Symlink { .. }), "{err}");
        });
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_board_file_is_refused() {
        with_temp_dir(|| {
            let board = create_board("Real".into()).unwrap();
            let outside = paths::base_dir().join("outside.json");
            fs::write(&outside, "{\"name\":\"Planted\"}").unwrap();
            let meta_path = paths::board_meta_path(&board.id);
            fs::remove_file(&meta_path).unwrap();
            std::os::unix::fs::symlink(&outside, &meta_path).unwrap();

            let err = load_board(&board.id).unwrap_err();
            assert!(matches!(err, StorageError::Symlink { .. }), "{err}");
        });
    }

    #[test]
    fn saved_boards_do_not_carry_an_id() {
        with_temp_dir(|| {
            let board = create_board("Board".into()).unwrap();

            let raw = fs::read_to_string(paths::board_meta_path(&board.id)).unwrap();
            let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
            assert!(json.get("id").is_none(), "id should not be persisted: {raw}");
            assert_eq!(load_board(&board.id).unwrap().id, board.id);
        });
    }

    #[test]
    fn directories_whose_name_is_not_an_id_are_not_boards() {
        with_temp_dir(|| {
            let real = create_board("Real".into()).unwrap();
            write_board_json("not an id", "not an id");

            let listed = list_boards().unwrap();
            assert_eq!(listed.len(), 1);
            assert_eq!(listed[0].id, real.id);
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
