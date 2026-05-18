//! Shared helpers for unit tests across the crate.
//!
//! Only compiled in `cfg(test)` builds — never linked into release binaries.

use std::env;

use crate::storage::board_store;

/// Set `TCT_DATA_DIR` to a fresh `TempDir` for the duration of `f`.
///
/// The temp dir is dropped (and cleaned up) when `f` returns. The env var
/// is restored to unset afterwards. Because `TCT_DATA_DIR` is process-global,
/// tests using this helper must run single-threaded (`--test-threads=1`,
/// already required by the project).
pub fn with_temp_dir<F: FnOnce()>(f: F) {
    let dir = tempfile::tempdir().unwrap();
    // SAFETY: tests are single-threaded; env mutation only races with
    // other tests we control.
    unsafe { env::set_var("TCT_DATA_DIR", dir.path()) };
    board_store::ensure_base_dirs().unwrap();
    f();
    unsafe { env::remove_var("TCT_DATA_DIR") };
}
