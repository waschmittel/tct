//! Shared helpers for unit tests across the crate.
//!
//! Only compiled in `cfg(test)` builds — never linked into release binaries.

use std::env;

use crate::model::board::{BoardMeta, ListMeta};
use crate::model::card::Card;
use crate::model::id::{BoardId, ListId};
use crate::storage::{board_store, card_store};

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
    board_store::open_data_dir().unwrap();
    f();
    unsafe { env::remove_var("TCT_DATA_DIR") };
}

/// Append a List to `meta` and persist the Board. Returns the List's id.
pub fn add_list(meta: &mut BoardMeta, name: &str) -> ListId {
    let id = ListId::generate();
    meta.lists.push(ListMeta { id: id.clone(), name: name.to_string(), archived: false });
    board_store::save_board(meta).unwrap();
    id
}

/// Persist `cards` as the contents of `list_id`, in the given order.
///
/// Stamps membership and rank onto the caller's Cards too, so assertions can
/// compare against them without re-reading.
pub fn put_cards(board_id: &BoardId, list_id: &ListId, cards: &mut [Card]) {
    for (idx, card) in cards.iter_mut().enumerate() {
        card.list_id = list_id.clone();
        card.position = (idx + 1) as f64;
        card_store::save_card(board_id, card).unwrap();
    }
}
