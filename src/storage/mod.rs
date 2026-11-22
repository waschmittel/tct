pub mod board_store;
pub mod card_store;
pub mod paths;
pub mod version;

use std::fs;
use std::io::{self, Write};
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
    #[error("refusing to follow a symlink: {path}\n  hint: {hint}")]
    Symlink { path: String, hint: &'static str },
    #[error(
        "data directory is version {found}, but this tct reads version {supported}\n  hint: it was written by a newer tct. Upgrade, or point TCT_DATA_DIR at a different directory."
    )]
    VersionTooNew { found: u32, supported: u32 },
    #[error(
        "data directory is version {found}, but this tct reads version {supported}\n  hint: the format it uses is no longer readable. Convert it with the tct version that wrote it, or start a fresh data directory."
    )]
    VersionTooOld { found: u32, supported: u32 },
    #[error(
        "unreadable version file: {found:?}\n  hint: .tct/.version must hold a single integer."
    )]
    VersionUnreadable { found: String },
    #[error(
        "tct data directory {path} does not exist\n  hint: run `tct init` to create it (or point TCT_DATA_DIR at an existing directory)."
    )]
    NotInitialized { path: String },
    #[error("Board not found: {0}")]
    BoardNotFound(String),
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

const SYMLINK_HINT: &str = "everything under the data directory must be a real file or \
directory. Validated ids keep a path from naming somewhere outside the data dir, but a symlink \
redirects in the filesystem instead, where the path cannot show it — so tct does not follow one. \
Replace the link with the file itself.";

/// Read a file that must not be a symlink.
///
/// `symlink_metadata` does not follow links, so this distinguishes "a file
/// here" from "a pointer to a file somewhere else". Every read under the data
/// directory goes through it: the data dir may be a checked-out repository, and
/// a repository can carry symlinks.
pub(crate) fn read_unlinked(path: &Path) -> Result<String> {
    reject_symlink(path)?;
    Ok(fs::read_to_string(path)?)
}

/// Fail if `path` is a symlink. Missing paths are not symlinks — callers that
/// care about existence check it themselves.
pub(crate) fn reject_symlink(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Err(StorageError::Symlink {
            path: path.display().to_string(),
            hint: SYMLINK_HINT,
        }),
        _ => Ok(()),
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

/// Write `contents` to `path` via a fresh sibling temp file and a rename.
///
/// The temp file has a random name and is created exclusively (`O_EXCL`, which
/// never follows a symlink) with mode 0600, so a symlink planted in a checked-out
/// data dir at a predictable name cannot redirect the write. It is synced before
/// the rename, so a crash leaves the old or the new file, never an empty one.
fn atomic_write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::other("path has no parent directory"))?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(contents)?;
    tmp.as_file().sync_all()?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

/// `create_dir_all` whose new directories are private (0700 on Unix): cards
/// can hold sensitive text, and the default 0755 would expose it on shared
/// machines. Existing directories keep their mode.
pub(crate) fn create_private_dir_all(path: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(path)
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
        let names: Vec<_> = fs::read_dir(dir.path()).unwrap().flatten().collect();
        assert_eq!(names.len(), 1, "only the target should remain");
    }

    /// A repository can plant a symlink at the old predictable temp names
    /// (`card-x.tmp`, `.version.tmp`); writing must never go through it.
    #[cfg(unix)]
    #[test]
    fn atomic_write_ignores_symlinks_planted_at_predictable_names() {
        let dir = tempdir().unwrap();
        let victim = dir.path().join("victim.txt");
        fs::write(&victim, "PRECIOUS").unwrap();
        for (target, planted) in [("card-x.json", "card-x.tmp"), (".version", ".version.tmp")] {
            std::os::unix::fs::symlink(&victim, dir.path().join(planted)).unwrap();
            atomic_write(&dir.path().join(target), b"new").unwrap();
        }
        assert_eq!(fs::read_to_string(&victim).unwrap(), "PRECIOUS");
        assert_eq!(
            fs::read_to_string(dir.path().join(".version")).unwrap(),
            "new"
        );
    }

    #[cfg(unix)]
    #[test]
    fn written_files_and_new_dirs_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let sub = dir.path().join("a/b");
        create_private_dir_all(&sub).unwrap();
        let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&sub), 0o700);
        let file = sub.join("f.json");
        atomic_write(&file, b"x").unwrap();
        assert_eq!(mode(&file), 0o600);
    }

    #[test]
    fn atomic_write_fails_on_missing_parent() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("missing_subdir/data.json");
        let err = atomic_write(&target, b"x");
        assert!(err.is_err());
    }
}
