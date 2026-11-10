#[cfg(any(debug_assertions, test))]
use std::collections::HashSet;

use super::*;
#[cfg(any(debug_assertions, test))]
use crate::model::id::ListId;

impl BoardEditor {
    /// Verify the board's structural invariants, returning `Err` with the first
    /// violation found. Run after `load` (post-repair) and after every `apply`
    /// in debug builds, so a command that corrupts state fails loudly at the
    /// point of introduction instead of surfacing later as a mystery bug (e.g.
    /// the move-over-archived index-space regression — see ADR-0006). It is a
    /// dev/test net, not a runtime guard: release builds skip it.
    ///
    /// Invariants:
    /// 1. Every Card's `list_id` names a List in `meta.lists` (no orphan card —
    ///    an orphan would be invisible in every view, the bug 0006 fought).
    /// 2. Each active List's derived `card_ids` equals the canonical order
    ///    rebuilt from card `position`s (derived view in sync with positions).
    /// 3. `selected_card`/`scroll_offset` have one slot per active List,
    ///    `selected_list` is in range, and each `selected_card` ordinal is
    ///    within that List's visible-card count.
    ///
    /// Dangling `label_id`s are deliberately NOT checked: per 0006's lazy-cleanup
    /// decision, `DeleteLabel` leaves dead refs on cards until their next save,
    /// and `resolved_labels`/`matches_search` ignore unknown ids — tolerated
    /// state, not corruption (`repair_references` strips them on load).
    #[cfg(any(debug_assertions, test))]
    pub(super) fn check_invariants(&self) -> Result<(), String> {
        let b = &self.board;
        let valid_lists: HashSet<&ListId> = b.meta.lists.iter().map(|l| &l.id).collect();

        for card in b.cards.values() {
            if !valid_lists.contains(&card.list_id) {
                return Err(format!(
                    "card {} has list_id {:?} naming no known List",
                    card.id, card.list_id
                ));
            }
        }

        for list in &b.lists {
            let expected = crate::model::list::ordered_card_ids(&list.id, &b.cards);
            if list.card_ids != expected {
                return Err(format!(
                    "list {} derived card_ids diverged from position order",
                    list.id
                ));
            }
        }

        let n = b.lists.len();
        if b.selected_card.len() != n {
            return Err(format!(
                "selected_card has {} slots, expected {n}",
                b.selected_card.len()
            ));
        }
        if b.scroll_offset.len() != n {
            return Err(format!(
                "scroll_offset has {} slots, expected {n}",
                b.scroll_offset.len()
            ));
        }
        if n > 0 && b.selected_list >= n {
            return Err(format!(
                "selected_list {} out of range (lists: {n})",
                b.selected_list
            ));
        }
        for i in 0..n {
            let count = b.visible_card_count(i);
            let ord = b.selected_card[i];
            if count == 0 && ord != 0 {
                return Err(format!(
                    "list idx {i} has no visible cards but selected_card ordinal is {ord}"
                ));
            }
            if count > 0 && ord >= count {
                return Err(format!(
                    "list idx {i} selected_card ordinal {ord} >= visible count {count}"
                ));
            }
        }

        Ok(())
    }
}
