use super::*;
use crate::command::MoveDir;
use crate::model::id::{CardId, ListId};

impl BoardEditor {
    pub(super) fn move_card(
        &mut self,
        card_id: &CardId,
        direction: MoveDir,
        pending: &mut Vec<PendingWrite>,
    ) -> Result<(), BoardEditorError> {
        // Locate the list and position
        let (src_idx, ci) = self
            .board
            .lists
            .iter()
            .enumerate()
            .find_map(|(li, l)| {
                l.card_ids
                    .iter()
                    .position(|id| id == card_id)
                    .map(|ci| (li, ci))
            })
            .ok_or_else(|| BoardEditorError::NotFound {
                kind: "card",
                id: card_id.to_string(),
            })?;

        // Moves operate over the *visible* sequence (archived Cards are hidden,
        // see ADR-0006 / `visible_cards`), so a move past an interleaved archived
        // Card still shifts the Card one slot in the user-visible order. Only the
        // moved Card's `position` (and, cross-list, `list_id`) changes; affected
        // lists' derived `card_ids` are rebuilt from positions. One Card written.
        match direction {
            MoveDir::Up | MoveDir::Down => {
                let vis = self.board.visible_cards(src_idx, None);
                let Some(vp) = vis.iter().position(|&i| i == ci) else {
                    return Ok(());
                };
                let new_pos = match direction {
                    MoveDir::Up => {
                        if vp == 0 {
                            return Ok(());
                        }
                        let prev = (vp >= 2)
                            .then(|| self.pos_of_raw(src_idx, vis[vp - 2]))
                            .flatten();
                        let next = self.pos_of_raw(src_idx, vis[vp - 1]);
                        crate::model::rank::between(prev, next)
                    }
                    MoveDir::Down => {
                        if vp + 1 >= vis.len() {
                            return Ok(());
                        }
                        let prev = self.pos_of_raw(src_idx, vis[vp + 1]);
                        let next = vis.get(vp + 2).and_then(|&r| self.pos_of_raw(src_idx, r));
                        crate::model::rank::between(prev, next)
                    }
                    _ => unreachable!(),
                };
                if let Some(card) = self.board.cards.get_mut(card_id) {
                    card.position = new_pos;
                }
                self.rebuild_list(src_idx);
                if let Some(ord) = self.visible_ordinal(src_idx, card_id)
                    && let Some(slot) = self.board.selected_card.get_mut(src_idx)
                {
                    *slot = ord;
                }
                pending.push(PendingWrite::Card(Box::new(
                    self.board.cards[card_id].clone(),
                )));
            }
            MoveDir::Left | MoveDir::Right => {
                let dst = match direction {
                    MoveDir::Left if src_idx > 0 => src_idx - 1,
                    MoveDir::Right if src_idx + 1 < self.board.lists.len() => src_idx + 1,
                    _ => return Ok(()),
                };
                // Land at the same visible slot in the destination (clamped to
                // its visible length), between that slot's visible neighbors.
                let vp = self
                    .board
                    .visible_cards(src_idx, None)
                    .iter()
                    .position(|&i| i == ci)
                    .unwrap_or(0);
                let vis_dst = self.board.visible_cards(dst, None);
                let insert_vp = vp.min(vis_dst.len());
                let prev = (insert_vp > 0)
                    .then(|| self.pos_of_raw(dst, vis_dst[insert_vp - 1]))
                    .flatten();
                let next = vis_dst
                    .get(insert_vp)
                    .and_then(|&r| self.pos_of_raw(dst, r));
                let new_pos = crate::model::rank::between(prev, next);
                let dst_list_id = self.board.lists[dst].id.clone();
                if let Some(card) = self.board.cards.get_mut(card_id) {
                    card.list_id = dst_list_id;
                    card.position = new_pos;
                }
                self.rebuild_list(src_idx);
                self.rebuild_list(dst);
                self.board.selected_list = dst;
                if let Some(ord) = self.visible_ordinal(dst, card_id)
                    && let Some(slot) = self.board.selected_card.get_mut(dst)
                {
                    *slot = ord;
                }
                pending.push(PendingWrite::Card(Box::new(
                    self.board.cards[card_id].clone(),
                )));
            }
        }
        Ok(())
    }

    pub(super) fn move_list(
        &mut self,
        list_id: &ListId,
        direction: MoveDir,
        pending: &mut Vec<PendingWrite>,
    ) -> Result<(), BoardEditorError> {
        let i = self
            .board
            .lists
            .iter()
            .position(|l| &l.id == list_id)
            .ok_or_else(|| BoardEditorError::NotFound {
                kind: "list",
                id: list_id.to_string(),
            })?;
        let target = match direction {
            MoveDir::Left => {
                if i == 0 {
                    return Ok(());
                }
                i - 1
            }
            MoveDir::Right => {
                if i + 1 >= self.board.lists.len() {
                    return Ok(());
                }
                i + 1
            }
            _ => return Err(BoardEditorError::Invariant("MoveList only Left/Right")),
        };
        // Reflect the active-view swap in meta.lists by swapping the two list
        // ids there (archived entries may sit between them, so use their actual
        // positions in meta.lists).
        let id_i = self.board.lists[i].id.clone();
        let id_t = self.board.lists[target].id.clone();
        if let (Some(mi), Some(mt)) = (
            self.board.meta.lists.iter().position(|l| l.id == id_i),
            self.board.meta.lists.iter().position(|l| l.id == id_t),
        ) {
            self.board.meta.lists.swap(mi, mt);
        }
        self.board.lists.swap(i, target);
        self.board.selected_card.swap(i, target);
        self.board.scroll_offset.swap(i, target);
        if self.board.selected_list == i {
            self.board.selected_list = target;
        } else if self.board.selected_list == target {
            self.board.selected_list = i;
        }
        pending.push(PendingWrite::Board);
        Ok(())
    }
}
