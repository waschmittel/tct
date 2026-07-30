//! Every path under the data directory is built here, and every id-bearing
//! component takes a [`ShortId`] rather than a `&str`. Ids come from JSON the
//! process does not control, so this signature is the guarantee that a board or
//! card file cannot be made to resolve outside the data directory — the
//! charset that would be needed (`/`, `..`) cannot exist in a `ShortId`.

use std::path::PathBuf;

use crate::model::id::{BoardId, CardId};

/// The data directory: `TCT_DATA_DIR`, else the nearest `.tct` walking up from
/// the cwd, else `~/.tct`.
///
/// The discovered `.tct` must be a real directory. `TCT_DATA_DIR` and `~/.tct`
/// are the user's own say-so about where their data lives and may well be
/// symlinks into cloud storage, but a discovered `.tct` is repository content —
/// a checked-out `.tct -> /etc` would otherwise silently make `/etc` the data
/// directory.
pub fn base_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("TCT_DATA_DIR") {
        return PathBuf::from(dir);
    }
    if let Ok(cwd) = std::env::current_dir() {
        let mut dir = cwd.as_path();
        loop {
            let candidate = dir.join(".tct");
            if is_real_dir(&candidate) {
                return candidate;
            }
            match dir.parent() {
                Some(parent) => dir = parent,
                None => break,
            }
        }
    }
    dirs::home_dir()
        .expect("Cannot determine home directory")
        .join(".tct")
}

/// A directory and not a symlink to one — `symlink_metadata` does not follow.
fn is_real_dir(path: &std::path::Path) -> bool {
    std::fs::symlink_metadata(path).map(|m| m.is_dir()).unwrap_or(false)
}

/// Home-shortened display form of [`base_dir`], e.g. `~/.tct`.
pub fn display_base_dir() -> String {
    let dir = base_dir();
    if let Some(home) = dirs::home_dir()
        && let Ok(rest) = dir.strip_prefix(&home)
    {
        return format!("~/{}", rest.display());
    }
    dir.display().to_string()
}

pub fn boards_dir() -> PathBuf {
    base_dir().join("boards")
}

pub fn board_order_path() -> PathBuf {
    base_dir().join("board_order.json")
}

/// The empty id is legal as a Card's "unassigned" `list_id`, but it is not a
/// directory name — joining it yields the boards directory itself, which would
/// make `delete_board` a recursive delete of every board. No board or card id
/// can be empty (both come from a real path component), so this only guards
/// against a future caller inventing one.
pub fn board_dir(board_id: &BoardId) -> PathBuf {
    debug_assert!(!board_id.is_empty(), "the empty id does not name a board directory");
    boards_dir().join(board_id.as_str())
}

pub fn board_meta_path(board_id: &BoardId) -> PathBuf {
    board_dir(board_id).join("board.json")
}

#[cfg(test)]
pub fn list_path(board_id: &BoardId, list_id: &crate::model::id::ListId) -> PathBuf {
    board_dir(board_id).join(format!("list-{list_id}.json"))
}

pub fn card_path(board_id: &BoardId, card_id: &CardId) -> PathBuf {
    board_dir(board_id).join(format!("card-{card_id}.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::with_temp_dir;

    #[test]
    fn base_dir_uses_env_override() {
        with_temp_dir(|| {
            let expected = std::env::var("TCT_DATA_DIR").unwrap();
            assert_eq!(base_dir(), PathBuf::from(expected));
        });
    }

    #[test]
    fn boards_dir_under_base() {
        with_temp_dir(|| {
            assert_eq!(boards_dir(), base_dir().join("boards"));
        });
    }

    #[test]
    fn board_order_path_under_base() {
        with_temp_dir(|| {
            assert_eq!(board_order_path(), base_dir().join("board_order.json"));
        });
    }

    #[test]
    fn board_dir_uses_board_id() {
        with_temp_dir(|| {
            assert_eq!(board_dir(&"abc123".into()), boards_dir().join("abc123"));
        });
    }

    #[test]
    fn list_path_format() {
        with_temp_dir(|| {
            let p = list_path(&"brd1".into(), &"lst1".into());
            assert!(p.ends_with("list-lst1.json"));
            assert!(p.to_string_lossy().contains("brd1"));
        });
    }

    #[test]
    fn card_path_format() {
        with_temp_dir(|| {
            let p = card_path(&"brd1".into(), &"crd1".into());
            assert!(p.ends_with("card-crd1.json"));
        });
    }

    #[test]
    fn board_meta_path_format() {
        with_temp_dir(|| {
            assert!(board_meta_path(&"brd1".into()).ends_with("board.json"));
        });
    }

    /// Both builders interpolate an id into a path, so an id able to carry a
    /// path separator would add components and escape. No id that `ShortId`
    /// admits can — including the awkward ones.
    #[test]
    fn no_constructible_id_adds_a_path_component() {
        with_temp_dir(|| {
            let board: BoardId = "brd1".into();
            for edge in ["-", "_", "a", &"z".repeat(crate::model::id::MAX_LEN)] {
                let card = CardId::parse(edge).unwrap();
                let board_edge = BoardId::parse(edge).unwrap();
                assert_eq!(board_dir(&board_edge).parent().unwrap(), boards_dir());
                assert_eq!(card_path(&board, &card).parent().unwrap(), board_dir(&board));
            }
        });
    }

    /// A repository can carry symlinks, so a checked-out `.tct -> /etc` would
    /// otherwise make `/etc` the data directory.
    #[cfg(unix)]
    #[test]
    fn a_symlinked_tct_is_not_adopted_by_discovery() {
        let outside = tempfile::tempdir().unwrap();
        let repo = tempfile::tempdir().unwrap();
        // `current_dir` reports the resolved path (on macOS /var → /private/var),
        // so compare against the same form or the assertion holds vacuously.
        let repo_path = std::fs::canonicalize(repo.path()).unwrap();
        std::os::unix::fs::symlink(outside.path(), repo_path.join(".tct")).unwrap();

        let restore = std::env::current_dir().unwrap();
        let data_dir = std::env::var("TCT_DATA_DIR").ok();
        unsafe { std::env::remove_var("TCT_DATA_DIR") };
        std::env::set_current_dir(&repo_path).unwrap();

        let adopted = base_dir();

        std::env::set_current_dir(restore).unwrap();
        if let Some(dir) = data_dir {
            unsafe { std::env::set_var("TCT_DATA_DIR", dir) };
        }
        assert!(!adopted.starts_with(&repo_path), "adopted the repo's symlinked .tct: {adopted:?}");
    }

    /// The empty id is the one `ShortId` admits that is not a path component:
    /// it names the boards directory rather than a board inside it.
    #[test]
    fn the_empty_id_does_not_name_a_board_directory() {
        with_temp_dir(|| {
            let empty = BoardId::default();
            assert_eq!(boards_dir().join(empty.as_str()).parent().unwrap(), base_dir());
        });
    }
}
