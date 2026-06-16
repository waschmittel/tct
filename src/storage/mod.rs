pub mod board_store;
pub mod card_store;
/// Legacy per-List file storage. After the card-owned-membership migration
/// (see [`migrate`]) no `list-*.json` files are written; this module survives
/// only to build legacy fixtures in tests.
#[cfg(test)]
pub mod list_store;
pub mod migrate;
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
    #[error("corrupt file: {path}\n  parse error: {source}\n  hint: {hint}")]
    Corrupt {
        path: String,
        source: serde_json::Error,
        hint: &'static str,
    },
    #[error("Board not found: {0}")]
    BoardNotFound(String),
    #[cfg(test)]
    #[error("List not found: {0}")]
    ListNotFound(String),
    #[error("Card not found: {0}")]
    CardNotFound(String),
}

pub type Result<T> = std::result::Result<T, StorageError>;

const CORRUPT_HINT: &str = "the file is not valid JSON for its schema — it may have been \
hand-edited, truncated, or written by an incompatible version. Fix the JSON or restore from \
backup. To skip unreadable files and load the rest, set TCT_SKIP_CORRUPT=1.";

/// Build a [`StorageError::Corrupt`] that names the offending file, the
/// underlying serde message (which carries line:column), and an actionable hint.
pub(crate) fn corrupt(path: &Path, source: serde_json::Error) -> StorageError {
    StorageError::Corrupt {
        path: path.display().to_string(),
        source,
        hint: CORRUPT_HINT,
    }
}

/// When set (`TCT_SKIP_CORRUPT` to a non-empty, non-`0` value), loaders that
/// iterate a directory drop unreadable files and return partial data instead of
/// failing the whole load. Single-file loads still fail — there is nothing to
/// return partial of.
pub(crate) fn skip_corrupt() -> bool {
    std::env::var("TCT_SKIP_CORRUPT")
        .map(|v| !v.is_empty() && v != "0")
        .unwrap_or(false)
}

fn atomic_write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, contents)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn atomic_write_creates_file() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("data.json");
        atomic_write(&target, b"hello").unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "hello");
    }

    #[test]
    fn atomic_write_overwrites_existing() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("data.json");
        fs::write(&target, "old").unwrap();
        atomic_write(&target, b"new").unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "new");
    }

    #[test]
    fn atomic_write_no_tmp_orphan_on_success() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("file.json");
        atomic_write(&target, b"x").unwrap();
        let tmp = target.with_extension("tmp");
        assert!(!tmp.exists(), "tmp file should not remain after success");
    }

    #[test]
    fn atomic_write_fails_on_missing_parent() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("missing_subdir/data.json");
        let err = atomic_write(&target, b"x");
        assert!(err.is_err());
    }
}
