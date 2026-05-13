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
