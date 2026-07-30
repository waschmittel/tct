use std::fs;

use crate::errln;
use crate::model::board::BoardMeta;
use crate::model::id::BoardId;

use super::paths;
use super::{atomic_write, Result, StorageError};

/// Prepare the data directory for use: create it if new, then check its
/// format version. Every entry point already called this before touching
/// storage, so the version gate cannot be forgotten by a future one.
pub fn open_data_dir() -> Result<()> {
    fs::create_dir_all(paths::boards_dir())?;
    super::version::ensure_compatible()
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

/// Active Boards in display order: ascending `position`, ties broken by
/// creation time then id so the order is total and stable. Boards written
/// before positions existed all sit at 0.0 and fall back to creation order.
pub fn list_boards() -> Result<Vec<BoardMeta>> {
    let mut boards: Vec<BoardMeta> = read_all_boards()?
        .into_iter()
        .filter(|meta| !meta.archived)
        .collect();
    boards.sort_by(|a, b| {
        crate::model::rank::cmp(a.position, b.position)
            .then(a.created_at.cmp(&b.created_at))
            .then(a.id.cmp(&b.id))
    });
    Ok(boards)
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
    fn list_boards_orders_by_position() {
        with_temp_dir(|| {
            for (name, position) in [("Alpha", 3.0), ("Beta", 1.0), ("Gamma", 2.0)] {
                let mut meta = BoardMeta::new(name.into());
                meta.position = position;
                save_board(&meta).unwrap();
            }

            let listed = list_boards().unwrap();

            let names: Vec<_> = listed.iter().map(|b| b.name.as_str()).collect();
            assert_eq!(names, ["Beta", "Gamma", "Alpha"]);
        });
    }

    /// Boards written before ranks existed all deserialize to 0.0, so the
    /// order has to stay total and stable on its own.
    #[test]
    fn tied_positions_fall_back_to_creation_order() {
        with_temp_dir(|| {
            let base = chrono::Utc::now();
            for (name, minutes) in [("Third", 2), ("First", 0), ("Second", 1)] {
                let mut meta = BoardMeta::new(name.into());
                meta.position = 0.0;
                meta.created_at = base + chrono::Duration::minutes(minutes);
                save_board(&meta).unwrap();
            }

            let listed = list_boards().unwrap();

            let names: Vec<_> = listed.iter().map(|b| b.name.as_str()).collect();
            assert_eq!(names, ["First", "Second", "Third"]);
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
