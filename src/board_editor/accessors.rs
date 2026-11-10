use super::*;
use crate::model::card::Card;
use crate::model::id::{CardId, ListId};
use crate::model::list::CardList;
use crate::storage::{board_store, card_store};

impl BoardEditor {
    /// Stage a card into the in-memory model without staging a write. Used
    /// when callers need to bring an archived card into scope before
    /// applying a command that touches it (e.g. `RestoreCard`).
    pub fn with_extra_card(&mut self, card: Card) {
        self.board.cards.insert(card.id.clone(), card);
    }

    /// Archived Cards of this board, read from disk. Errors if a card file is
    /// corrupt (unless `TCT_SKIP_CORRUPT` is set).
    pub fn archived_cards(&self) -> Result<Vec<Card>, BoardEditorError> {
        Ok(card_store::list_archived_cards(&self.board.meta.id)?)
    }

    /// Archived Lists of this board, built from the board's `ListMeta`s and the
    /// in-memory Card map (Cards keep their `list_id` when their List is archived).
    pub fn archived_lists(&self) -> Vec<CardList> {
        crate::model::list::build_lists(&self.board.meta, &self.board.cards, true)
    }

    /// Permanently delete an archived Card's file. The only hard delete on
    /// Cards — everything else is Archive.
    pub fn delete_archived_card(&mut self, card_id: &CardId) -> Result<(), BoardEditorError> {
        card_store::delete_card(&self.board.meta.id, card_id)?;
        self.board.cards.remove(card_id);
        Ok(())
    }

    /// Permanently delete an archived List along with its Cards: removes the
    /// Card files and the `ListMeta`, then persists the board.
    pub fn delete_archived_list(
        &mut self,
        list_id: &ListId,
        card_ids: &[CardId],
    ) -> Result<(), BoardEditorError> {
        for cid in card_ids {
            let _ = card_store::delete_card(&self.board.meta.id, cid);
            self.board.cards.remove(cid);
        }
        self.board.meta.lists.retain(|l| &l.id != list_id);
        board_store::save_board(&self.board.meta)?;
        Ok(())
    }

    /// Most recently added card id, set by `Command::AddCard`. Cleared at the
    /// start of every `apply()` call so callers see only this run's result.
    pub fn last_added_card_id(&self) -> Option<&CardId> {
        self.last_added_card_id.as_ref()
    }
}
