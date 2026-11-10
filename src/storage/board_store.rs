use std::fs;

use crate::errln;
use crate::model::board::BoardMeta;
use crate::model::id::BoardId;

use super::paths;
use super::{Result, StorageError, atomic_write};

/// Written into every data directory [`create_data_dir`] sets up; kept as a
/// file for easy editing.
const README: &str = include_str!("../../assets/data-dir-README.md");

/// Prepare the data directory for use, then check its format version. Every
/// entry point already called this before touching storage, so the version
/// gate cannot be forgotten by a future one.
///
/// A missing directory is created only when it is the `~/.tct` fallback; one
/// chosen by `TCT_DATA_DIR` is [`StorageError::NotInitialized`] — a typo must
/// not start a fresh data dir, `tct init` creates it deliberately.
pub fn open_data_dir() -> Result<()> {
    let dir = paths::base_dir();
    if dir.is_dir() {
        fs::create_dir_all(paths::boards_dir())?;
        super::version::ensure_compatible()
    } else if paths::is_home_fallback() {
        create_data_dir(&dir)
    } else {
        Err(StorageError::NotInitialized {
            path: dir.display().to_string(),
        })
    }
}

/// `tct init`: create the data directory at [`paths::base_dir`] (and its README
/// if absent). Returns `true` if the directory already existed.
pub fn init_data_dir() -> Result<bool> {
    let dir = paths::base_dir();
    let existed = match fs::symlink_metadata(&dir) {
        Ok(meta) if meta.is_dir() => true,
        Ok(_) => {
            return Err(StorageError::Io(std::io::Error::other(format!(
                "{} exists and is not a directory",
                dir.display()
            ))));
        }
        Err(_) => false,
    };
    create_data_dir(&dir)?;
    Ok(existed)
}

/// The one place a data directory is set up — `tct init` and the implicit
/// `~/.tct` both come through here: the directory, `boards/`, the version
/// stamp, and the README. An existing README is never replaced, not even a
/// symlink, which `symlink_metadata` reports instead of following.
fn create_data_dir(dir: &std::path::Path) -> Result<()> {
    fs::create_dir_all(paths::boards_dir())?;
    super::version::ensure_compatible()?;
    let readme = dir.join("README.md");
    if fs::symlink_metadata(&readme).is_err() {
        atomic_write(&readme, README.as_bytes())?;
    }
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
    parse_board(board_id, &data).map_err(|e| super::corrupt(&path, e))
}

/// Parse a `board.json` text, stamping the id from its directory name. Shared
/// with `git_sync`, which reads old versions out of git.
pub(crate) fn parse_board(board_id: &BoardId, data: &str) -> serde_json::Result<BoardMeta> {
    let mut meta: BoardMeta = serde_json::from_str(data)?;
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
    debug_assert!(
        !meta.id.is_empty(),
        "board id must be stamped before saving"
    );
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

    #[test]
    fn a_missing_data_dir_needs_init_and_is_not_created() {
        with_temp_dir(|| {
            let missing = paths::base_dir().join("typo");
            unsafe { std::env::set_var("TCT_DATA_DIR", &missing) };

            assert!(matches!(
                open_data_dir(),
                Err(StorageError::NotInitialized { .. })
            ));
            assert!(!missing.exists());

            init_data_dir().unwrap();
            assert!(missing.join("boards").is_dir());
            open_data_dir().unwrap();
        });
    }

    #[test]
    fn implicitly_created_home_dir_gets_the_readme() {
        let home = tempfile::tempdir().unwrap();
        let (old_home, old_data) = (std::env::var_os("HOME"), std::env::var_os("TCT_DATA_DIR"));
        let old_cwd = std::env::current_dir().unwrap();
        // SAFETY: tests run single-threaded (`--test-threads=1`).
        unsafe {
            std::env::set_var("HOME", home.path());
            std::env::remove_var("TCT_DATA_DIR");
        }
        std::env::set_current_dir(home.path()).unwrap();

        let result = open_data_dir();
        let again = open_data_dir();
        let readme = home.path().join(".tct/README.md");
        let first = fs::read_to_string(&readme);

        std::env::set_current_dir(old_cwd).unwrap();
        unsafe {
            match old_home {
                Some(h) => std::env::set_var("HOME", h),
                None => std::env::remove_var("HOME"),
            }
            if let Some(d) = old_data {
                std::env::set_var("TCT_DATA_DIR", d)
            }
        }
        result.unwrap();
        again.unwrap();
        assert!(
            first
                .unwrap()
                .contains("https://github.com/waschmittel/tct")
        );
    }

    #[test]
    fn init_writes_readme_once_and_never_overwrites() {
        with_temp_dir(|| {
            assert!(init_data_dir().unwrap());
            let readme = paths::base_dir().join("README.md");
            assert!(
                fs::read_to_string(&readme)
                    .unwrap()
                    .contains("https://github.com/waschmittel/tct")
            );

            fs::write(&readme, "mine").unwrap();
            init_data_dir().unwrap();
            assert_eq!(fs::read_to_string(&readme).unwrap(), "mine");
        });
    }
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
    fn date_stamp_settings_persist_and_default_off() {
        with_temp_dir(|| {
            let mut meta = BoardMeta::new("Dates".into());
            save_board(&meta).unwrap();
            let loaded = load_board(&meta.id).unwrap();
            assert!(!loaded.show_created_date && !loaded.show_updated_date);

            meta.show_created_date = true;
            meta.show_updated_date = true;
            save_board(&meta).unwrap();
            let loaded = load_board(&meta.id).unwrap();
            assert!(loaded.show_created_date && loaded.show_updated_date);
        });
    }

    #[test]
    fn board_json_without_date_stamp_settings_loads_with_them_off() {
        with_temp_dir(|| {
            let meta = BoardMeta::new("Old".into());
            save_board(&meta).unwrap();
            let path = crate::storage::paths::board_dir(&meta.id).join("board.json");
            let mut value: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            let obj = value.as_object_mut().unwrap();
            obj.remove("show_created_date");
            obj.remove("show_updated_date");
            std::fs::write(&path, value.to_string()).unwrap();
            let loaded = load_board(&meta.id).unwrap();
            assert!(!loaded.show_created_date && !loaded.show_updated_date);
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
        fs::write(
            board_dir.join("board.json"),
            serde_json::to_string_pretty(&json).unwrap(),
        )
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
            assert!(
                json.get("id").is_none(),
                "id should not be persisted: {raw}"
            );
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
