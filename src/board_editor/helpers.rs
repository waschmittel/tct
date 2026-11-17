use super::*;
use crate::model::card::{Card, HistoryField};
use crate::model::id::{CardId, LabelId, ListId};
use crate::storage::{board_store, card_store};

impl BoardEditor {
    /// Fractional rank one past the last Card currently in `list_id`.
    pub(super) fn next_position(&self, list_id: &ListId) -> f64 {
        crate::model::rank::after_last(
            self.board
                .cards
                .values()
                .filter(|c| c.list_id == *list_id)
                .map(|c| c.position),
        )
    }

    /// Fractional rank one before the first Card currently in `list_id`.
    pub(super) fn first_position(&self, list_id: &ListId) -> f64 {
        crate::model::rank::before_first(
            self.board
                .cards
                .values()
                .filter(|c| c.list_id == *list_id)
                .map(|c| c.position),
        )
    }

    /// `position` of the Card at raw `card_ids` index `idx` in list `list_idx`.
    pub(super) fn pos_of_raw(&self, list_idx: usize, idx: usize) -> Option<f64> {
        self.board.lists[list_idx]
            .card_ids
            .get(idx)
            .and_then(|id| self.board.cards.get(id))
            .map(|c| c.position)
    }

    /// Ordinal of `card_id` within list `list_idx`'s visible (non-archived)
    /// sequence — the index space of `selected_card`.
    pub(super) fn visible_ordinal(&self, list_idx: usize, card_id: &CardId) -> Option<usize> {
        let card_ids = &self.board.lists[list_idx].card_ids;
        self.board
            .visible_cards(list_idx, None)
            .into_iter()
            .position(|raw| card_ids[raw] == *card_id)
    }

    /// Re-derive a list's in-memory `card_ids` from the current Card positions,
    /// so the live view matches what a reload would produce.
    pub(super) fn rebuild_list(&mut self, list_idx: usize) {
        let lid = self.board.lists[list_idx].id.clone();
        self.board.lists[list_idx].card_ids =
            crate::model::list::ordered_card_ids(&lid, &self.board.cards);
    }

    pub(super) fn card_mut(&mut self, card_id: &CardId) -> Result<&mut Card, BoardEditorError> {
        self.board
            .cards
            .get_mut(card_id)
            .ok_or_else(|| BoardEditorError::NotFound {
                kind: "card",
                id: card_id.to_string(),
            })
    }

    /// Merges label `from` into `into`: every card carrying `from` gets `into`
    /// instead (once), then `from` is removed from the board.
    pub(super) fn merge_label(
        &mut self,
        from: &LabelId,
        into: &LabelId,
        pending: &mut Vec<PendingWrite>,
    ) -> Result<(), BoardEditorError> {
        let labels = &mut self.board.meta.labels;
        let pos = labels.iter().position(|l| l.id == *from).ok_or_else(|| {
            BoardEditorError::NotFound {
                kind: "label",
                id: from.to_string(),
            }
        })?;
        let from_name = labels.remove(pos).name;
        let labels = &self.board.meta.labels;
        let into_name = labels
            .iter()
            .find(|l| l.id == *into)
            .map_or_else(String::new, |l| l.name.clone());
        for card in self.board.cards.values_mut() {
            let Some(pos) = card.label_ids.iter().position(|id| id == from) else {
                continue;
            };
            let old = card.label_names_text(labels);
            if card.label_ids.contains(into) {
                card.label_ids.remove(pos);
            } else {
                card.label_ids[pos] = into.clone();
            }
            let new = card.label_names_text(labels);
            card.log_change(
                HistoryField::Labels,
                format!("Merged label '{from_name}' into '{into_name}'"),
                Some(old),
                Some(new),
            );
            pending.push(PendingWrite::Card(Box::new(card.clone())));
        }
        if self.board.label_filter.as_ref() == Some(from) {
            self.board.label_filter = Some(into.clone());
        }
        pending.push(PendingWrite::Board);
        Ok(())
    }

    /// Clears the label filter when its label no longer exists — deleted
    /// here or by a remote change picked up on reload — so it cannot hide
    /// every card.
    pub(super) fn drop_dead_label_filter(&mut self) {
        if let Some(id) = &self.board.label_filter
            && !self.board.meta.labels.iter().any(|l| l.id == *id)
        {
            self.board.label_filter = None;
        }
    }

    pub(super) fn commit_pending(&self, writes: Vec<PendingWrite>) -> Result<(), BoardEditorError> {
        for w in writes {
            match w {
                PendingWrite::Card(mut c) => {
                    // Lazy label cleanup: drop label_ids that no longer name a
                    // board label, so a card sheds dead refs on its next save.
                    c.label_ids
                        .retain(|id| self.board.meta.labels.iter().any(|l| &l.id == id));
                    card_store::save_card(&self.board.meta.id, &c)?
                }
                PendingWrite::Board => board_store::save_board(&self.board.meta)?,
            }
        }
        Ok(())
    }
}
