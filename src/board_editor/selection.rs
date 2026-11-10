use super::*;
use crate::model::id::CardId;

impl BoardEditor {
    // ── Selection verbs (ADR-0002: direct methods, not Commands) ──────────

    pub fn select_list_left(&mut self) {
        if self.board.selected_list > 0 {
            self.board.selected_list -= 1;
        }
    }

    pub fn select_list_right(&mut self) {
        if self.board.selected_list < self.board.lists.len().saturating_sub(1) {
            self.board.selected_list += 1;
        }
    }

    /// Move selection down to the next visible Card (next match when a
    /// search is active). `selected_card` is an ordinal into the non-archived
    /// visible cards; we step in raw-index space (so search can skip archived
    /// *and* non-matching cards) then convert back to that ordinal.
    pub fn select_card_down(&mut self, search: Option<&str>) {
        let li = self.board.selected_list;
        let base = self.board.visible_cards(li, None);
        let cur_ord = self.board.selected_card.get(li).copied().unwrap_or(0);
        let Some(&cur_raw) = base.get(cur_ord) else {
            return;
        };
        if let Some(&next_raw) = self
            .board
            .visible_cards(li, search)
            .iter()
            .find(|&&i| i > cur_raw)
            && let Some(ord) = base.iter().position(|&r| r == next_raw)
        {
            self.board.selected_card[li] = ord;
        }
    }

    /// Move selection up to the previous visible Card (previous match when
    /// a search is active). See [`select_card_down`] for the ordinal handling.
    pub fn select_card_up(&mut self, search: Option<&str>) {
        let li = self.board.selected_list;
        let base = self.board.visible_cards(li, None);
        let cur_ord = self.board.selected_card.get(li).copied().unwrap_or(0);
        let Some(&cur_raw) = base.get(cur_ord) else {
            return;
        };
        if let Some(&prev_raw) = self
            .board
            .visible_cards(li, search)
            .iter()
            .rev()
            .find(|&&i| i < cur_raw)
            && let Some(ord) = base.iter().position(|&r| r == prev_raw)
        {
            self.board.selected_card[li] = ord;
        }
    }

    pub fn select_first_card(&mut self) {
        let li = self.board.selected_list;
        if let Some(slot) = self.board.selected_card.get_mut(li) {
            *slot = 0;
        }
    }

    pub fn select_last_card(&mut self) {
        let li = self.board.selected_list;
        let max = self.board.visible_card_count(li).saturating_sub(1);
        if let Some(slot) = self.board.selected_card.get_mut(li) {
            *slot = max;
        }
    }

    /// Jump to the first Card matching `query` anywhere on the board.
    pub fn select_first_match(&mut self, query: &str) {
        for li in 0..self.board.lists.len() {
            if let Some(&raw) = self.board.visible_cards(li, Some(query)).first() {
                self.board.selected_list = li;
                if let Some(ord) = self
                    .board
                    .visible_cards(li, None)
                    .iter()
                    .position(|&r| r == raw)
                {
                    self.board.selected_card[li] = ord;
                }
                return;
            }
        }
    }

    /// Focus the card with id `card_id`, if it exists and is visible (non-archived).
    /// If `search` is provided, the card must also match the active search.
    /// Returns true if the card was found and focused, or false if not found.
    pub fn focus_card(&mut self, card_id: &CardId, search: Option<&str>) -> bool {
        for (li, list) in self.board.lists.iter().enumerate() {
            let visible_all = self.board.visible_cards(li, None);
            let Some(ord) = visible_all
                .iter()
                .position(|&raw| list.card_ids.get(raw) == Some(card_id))
            else {
                continue;
            };
            if let Some(q) = search
                && !self
                    .board
                    .visible_cards(li, Some(q))
                    .contains(&visible_all[ord])
            {
                continue;
            }
            self.board.selected_list = li;
            self.board.selected_card[li] = ord;
            self.board.clamp_selection();
            return true;
        }
        false
    }

    // ── Card Detail cursor verbs ───────────────────────────────────────────

    pub fn reset_detail_cursor(&mut self) {
        self.board.detail_item_idx = 0;
        self.board.detail_scroll = 0;
    }

    pub fn detail_item_up(&mut self) {
        if self.board.detail_item_idx > 0 {
            self.board.detail_item_idx -= 1;
        }
    }

    /// Move the detail cursor down, clamped to the current Card's checklist.
    pub fn detail_item_down(&mut self) {
        let len = self
            .board
            .current_card()
            .map(|c| c.checklist.len())
            .unwrap_or(0);
        if self.board.detail_item_idx < len.saturating_sub(1) {
            self.board.detail_item_idx += 1;
        }
    }

    /// Scroll the description pane by `step`, clamped to `max_scroll`
    /// (reported by the renderer). The stored value is clamped first so a
    /// position that ran past the rendered max (e.g. after a layout change)
    /// responds to the very next key press instead of eating presses.
    pub fn scroll_detail(&mut self, step: usize, down: bool, max_scroll: usize) {
        let current = self.board.detail_scroll.min(max_scroll);
        self.board.detail_scroll = if down {
            (current + step).min(max_scroll)
        } else {
            current.saturating_sub(step)
        };
    }
}
