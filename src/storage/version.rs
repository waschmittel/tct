//! The persistence format version of a data directory.
//!
//! The version lives in one file, `.tct/.version`, and is checked once at
//! startup. That is deliberately the *only* place in the codebase that knows
//! about format history: the loaders parse exactly one shape and nothing else,
//! so a legacy field can never linger on a domain type and a legacy shape can
//! never be smuggled through a hot path (ADR-0009).
//!
//! Refusing to open a directory written by a newer tct is the point of the
//! file. Formats change by *removing* things — `card-<id>.json` no longer
//! carries an `id` (ADR-0007) — so an older binary reading a newer directory
//! would otherwise fail deep in serde with a message about a missing field,
//! or worse, load a partial record and write it back.

use std::path::Path;

use super::{paths, read_unlinked, Result, StorageError};

/// The format this binary reads and writes.
///
/// 1 — Boards are directories, Cards are `card-<id>.json` files, ids are the
/// path, Lists live inline in `board.json`, Board order is a per-Board
/// `position`.
pub const CURRENT: u32 = 1;

/// Check the data directory's version, stamping it when absent.
///
/// Absent means one of two things, and both are version [`CURRENT`]: a fresh
/// directory, or one written before the version file existed — every such
/// directory holds the v1 layout, because the formats that predate it are no
/// longer readable at all (ADR-0009).
pub fn ensure_compatible() -> Result<()> {
    let path = paths::version_path();
    match read_version(&path)? {
        Some(CURRENT) => Ok(()),
        Some(found) if found > CURRENT => Err(StorageError::VersionTooNew {
            found,
            supported: CURRENT,
        }),
        Some(found) => Err(StorageError::VersionTooOld {
            found,
            supported: CURRENT,
        }),
        None => stamp(),
    }
}

fn read_version(path: &Path) -> Result<Option<u32>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = read_unlinked(path)?;
    raw.trim()
        .parse::<u32>()
        .map(Some)
        .map_err(|_| StorageError::VersionUnreadable { found: raw.trim().to_string() })
}

fn stamp() -> Result<()> {
    super::atomic_write(&paths::version_path(), CURRENT.to_string().as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::with_temp_dir;

    #[test]
    fn stamps_a_directory_that_has_no_version() {
        with_temp_dir(|| {
            std::fs::remove_file(paths::version_path()).ok();

            ensure_compatible().unwrap();

            assert_eq!(std::fs::read_to_string(paths::version_path()).unwrap(), "1");
        });
    }

    #[test]
    fn accepts_the_current_version() {
        with_temp_dir(|| {
            std::fs::write(paths::version_path(), "1").unwrap();
            ensure_compatible().unwrap();
        });
    }

    #[test]
    fn refuses_a_newer_directory() {
        with_temp_dir(|| {
            std::fs::write(paths::version_path(), "2").unwrap();

            let err = ensure_compatible().unwrap_err();

            assert!(matches!(err, StorageError::VersionTooNew { found: 2, .. }), "{err}");
        });
    }

    #[test]
    fn refuses_an_older_directory() {
        with_temp_dir(|| {
            std::fs::write(paths::version_path(), "0").unwrap();

            let err = ensure_compatible().unwrap_err();

            assert!(matches!(err, StorageError::VersionTooOld { found: 0, .. }), "{err}");
        });
    }

    #[test]
    fn refuses_a_version_that_is_not_a_number() {
        with_temp_dir(|| {
            std::fs::write(paths::version_path(), "; rm -rf /").unwrap();

            let err = ensure_compatible().unwrap_err();

            assert!(matches!(err, StorageError::VersionUnreadable { .. }), "{err}");
        });
    }

    #[cfg(unix)]
    #[test]
    fn refuses_a_symlinked_version_file() {
        with_temp_dir(|| {
            let outside = paths::base_dir().join("outside");
            std::fs::write(&outside, "1").unwrap();
            std::fs::remove_file(paths::version_path()).ok();
            std::os::unix::fs::symlink(&outside, paths::version_path()).unwrap();

            let err = ensure_compatible().unwrap_err();

            assert!(matches!(err, StorageError::Symlink { .. }), "{err}");
        });
    }
}
