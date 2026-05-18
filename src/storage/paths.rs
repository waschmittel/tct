use std::path::PathBuf;

pub fn base_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("TCT_DATA_DIR") {
        return PathBuf::from(dir);
    }
    if let Ok(cwd) = std::env::current_dir() {
        let mut dir = cwd.as_path();
        loop {
            let candidate = dir.join(".tct");
            if candidate.is_dir() {
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

pub fn boards_dir() -> PathBuf {
    base_dir().join("boards")
}

pub fn board_order_path() -> PathBuf {
    base_dir().join("board_order.json")
}

pub fn board_dir(board_id: &str) -> PathBuf {
    boards_dir().join(board_id)
}

pub fn board_meta_path(board_id: &str) -> PathBuf {
    board_dir(board_id).join("board.json")
}

pub fn list_path(board_id: &str, list_id: &str) -> PathBuf {
    board_dir(board_id).join(format!("list-{list_id}.json"))
}

pub fn card_path(board_id: &str, card_id: &str) -> PathBuf {
    board_dir(board_id).join(format!("card-{card_id}.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn base_dir_uses_env_override() {
        let tmp = tempfile::tempdir().unwrap();
        unsafe { env::set_var("TCT_DATA_DIR", tmp.path()) };
        assert_eq!(base_dir(), tmp.path());
        unsafe { env::remove_var("TCT_DATA_DIR") };
    }

    #[test]
    fn boards_dir_under_base() {
        let tmp = tempfile::tempdir().unwrap();
        unsafe { env::set_var("TCT_DATA_DIR", tmp.path()) };
        assert_eq!(boards_dir(), tmp.path().join("boards"));
        unsafe { env::remove_var("TCT_DATA_DIR") };
    }

    #[test]
    fn board_order_path_under_base() {
        let tmp = tempfile::tempdir().unwrap();
        unsafe { env::set_var("TCT_DATA_DIR", tmp.path()) };
        assert_eq!(board_order_path(), tmp.path().join("board_order.json"));
        unsafe { env::remove_var("TCT_DATA_DIR") };
    }

    #[test]
    fn board_dir_uses_board_id() {
        let tmp = tempfile::tempdir().unwrap();
        unsafe { env::set_var("TCT_DATA_DIR", tmp.path()) };
        assert_eq!(board_dir("abc123"), tmp.path().join("boards").join("abc123"));
        unsafe { env::remove_var("TCT_DATA_DIR") };
    }

    #[test]
    fn list_path_format() {
        let tmp = tempfile::tempdir().unwrap();
        unsafe { env::set_var("TCT_DATA_DIR", tmp.path()) };
        let p = list_path("brd1", "lst1");
        assert!(p.ends_with("list-lst1.json"));
        assert!(p.to_string_lossy().contains("brd1"));
        unsafe { env::remove_var("TCT_DATA_DIR") };
    }

    #[test]
    fn card_path_format() {
        let tmp = tempfile::tempdir().unwrap();
        unsafe { env::set_var("TCT_DATA_DIR", tmp.path()) };
        let p = card_path("brd1", "crd1");
        assert!(p.ends_with("card-crd1.json"));
        unsafe { env::remove_var("TCT_DATA_DIR") };
    }

    #[test]
    fn board_meta_path_format() {
        let tmp = tempfile::tempdir().unwrap();
        unsafe { env::set_var("TCT_DATA_DIR", tmp.path()) };
        let p = board_meta_path("brd1");
        assert!(p.ends_with("board.json"));
        unsafe { env::remove_var("TCT_DATA_DIR") };
    }
}
