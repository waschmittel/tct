//! Board Editor — aggregate root for one Loaded Board.
//!
//! See `docs/adr/0001-board-editor-aggregate.md`. Owns the in-memory board
//! state; mutations enter via `apply(Command)` which mutates, appends the
//! History Entry, and stages writes for `commit_pending`. Selection State
//! is mutated only through the selection verbs below (ADR-0002).

use crate::app::LoadedBoard;
use crate::model::card::Card;
use crate::model::id::{BoardId, CardId};
use crate::storage::{self, board_store, card_store};
use repair::*;

mod accessors;
mod apply;
mod helpers;
mod invariants;
mod moves;
mod repair;
mod selection;
#[cfg(test)]
mod tests;

#[derive(Debug, thiserror::Error)]
pub enum BoardEditorError {
    #[error("not found: {kind} {id}")]
    NotFound { kind: &'static str, id: String },
    #[error("invariant violated: {0}")]
    Invariant(&'static str),
    #[error(transparent)]
    Storage(#[from] storage::StorageError),
}

pub struct BoardEditor {
    board: LoadedBoard,
    last_added_card_id: Option<CardId>,
    /// Non-fatal issues found and auto-repaired while loading the board (orphan
    /// cards reattached, dangling label refs stripped). Surfaced to the user as
    /// a status message; empty on a clean load.
    pub diagnostics: Vec<String>,
}

enum PendingWrite {
    Card(Box<Card>),
    Board,
}

impl BoardEditor {
    pub fn load(board_id: &BoardId) -> Result<Self, BoardEditorError> {
        // load_board migrates legacy boards in place before returning meta.
        let meta = board_store::load_board(board_id)?;
        let mut cards = card_store::load_all_cards(board_id)?;
        let diagnostics = repair_references(board_id, &meta, &mut cards);
        let lists = crate::model::list::build_lists(&meta, &cards, false);
        let num_lists = lists.len();
        let editor = Self {
            board: LoadedBoard {
                meta,
                lists,
                cards,
                selected_list: 0,
                selected_card: vec![0; num_lists],
                scroll_offset: (0..num_lists).map(|_| std::cell::Cell::new(0)).collect(),
                label_filter: None,
                detail_item_idx: 0,
                detail_scroll: 0,
                detail_max_scroll: std::cell::Cell::new(0),
            },
            last_added_card_id: None,
            diagnostics,
        };
        #[cfg(debug_assertions)]
        if let Err(e) = editor.check_invariants() {
            panic!("board invariant violated after load+repair: {e}");
        }
        Ok(editor)
    }

    /// Test-only constructor for in-memory fixtures (no disk).
    #[cfg(test)]
    pub fn from_loaded(board: LoadedBoard) -> Self {
        Self {
            board,
            last_added_card_id: None,
            diagnostics: Vec::new(),
        }
    }

    pub fn board(&self) -> &LoadedBoard {
        &self.board
    }

    /// Test-only escape hatch for fixtures that need to pre-arrange state.
    #[cfg(test)]
    pub fn board_mut(&mut self) -> &mut LoadedBoard {
        &mut self.board
    }

    /// Re-read the board from disk, preserving Selection State. Membership is
    /// rebuilt from each Card's `list_id`/`position`, so a partially-written
    /// card cannot orphan data here. An error means the board file is gone.
    pub fn reload(&mut self) -> Result<(), BoardEditorError> {
        let meta = board_store::load_board(&self.board.meta.id)?;
        let cards = card_store::load_all_cards(&self.board.meta.id)?;
        let lists = crate::model::list::build_lists(&meta, &cards, false);

        let num_lists = lists.len();
        let old_selected_list = self.board.selected_list;
        let old_selected_card = std::mem::take(&mut self.board.selected_card);
        let old_scroll_offset = std::mem::take(&mut self.board.scroll_offset);

        self.board.meta = meta;
        self.board.lists = lists;
        self.board.cards = cards;
        self.board.selected_card = vec![0; num_lists];
        self.board.scroll_offset = (0..num_lists)
            .map(|i| {
                let old = old_scroll_offset.get(i).map(|c| c.get()).unwrap_or(0);
                std::cell::Cell::new(old)
            })
            .collect();
        self.board.selected_list = old_selected_list.min(num_lists.saturating_sub(1));
        for i in 0..num_lists {
            if i < old_selected_card.len() {
                self.board.selected_card[i] = old_selected_card[i];
            }
        }
        self.drop_dead_label_filter();
        self.board.clamp_selection();
        Ok(())
    }
}
