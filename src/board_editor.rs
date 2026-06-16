//! Board Editor — aggregate root for one Loaded Board.
//!
//! See `docs/adr/0001-board-editor-aggregate.md`. Owns the in-memory board
//! state; mutations enter via `apply(Command)` which mutates, appends the
//! History Entry, and stages writes for `commit_pending`. Selection State
//! is mutated only through the selection verbs below (ADR-0002).

use crate::app::LoadedBoard;
use crate::command::{Command, MoveDir};
use crate::model::board::ListMeta;
use crate::model::card::{Card, ChecklistItem};
use crate::model::ids::ShortId;
use crate::model::label::Label;
use crate::model::list::CardList;
use crate::storage::{self, board_store, card_store};

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
    last_added_card_id: Option<ShortId>,
}

enum PendingWrite {
    Card(Card),
    Board,
}

impl BoardEditor {
    pub fn load(board_id: &str) -> Result<Self, BoardEditorError> {
        // load_board migrates legacy boards in place before returning meta.
        let meta = board_store::load_board(board_id)?;
        let cards = card_store::load_all_cards(board_id);
        let lists = crate::model::list::build_lists(&meta, &cards, false);
        let num_lists = lists.len();
        Ok(Self {
            board: LoadedBoard {
                meta,
                lists,
                cards,
                selected_list: 0,
                selected_card: vec![0; num_lists],
                scroll_offset: vec![0; num_lists],
                detail_item_idx: 0,
                detail_scroll: 0,
            },
            last_added_card_id: None,
        })
    }

    /// Test-only constructor for in-memory fixtures (no disk).
    #[cfg(test)]
    pub fn from_loaded(board: LoadedBoard) -> Self {
        Self { board, last_added_card_id: None }
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
        let cards = card_store::load_all_cards(&self.board.meta.id);
        let lists = crate::model::list::build_lists(&meta, &cards, false);

        let num_lists = lists.len();
        let old_selected_list = self.board.selected_list;
        let old_selected_card = std::mem::take(&mut self.board.selected_card);
        let old_scroll_offset = std::mem::take(&mut self.board.scroll_offset);

        self.board.meta = meta;
        self.board.lists = lists;
        self.board.cards = cards;
        self.board.selected_card = vec![0; num_lists];
        self.board.scroll_offset = vec![0; num_lists];
        self.board.selected_list = old_selected_list.min(num_lists.saturating_sub(1));
        for i in 0..num_lists {
            if i < old_selected_card.len() {
                self.board.selected_card[i] = old_selected_card[i];
            }
            if i < old_scroll_offset.len() {
                self.board.scroll_offset[i] = old_scroll_offset[i];
            }
        }
        self.board.clamp_selection();
        Ok(())
    }

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
    /// search is active).
    pub fn select_card_down(&mut self, search: Option<&str>) {
        let li = self.board.selected_list;
        let current = self.board.selected_card.get(li).copied().unwrap_or(0);
        let next = self
            .board
            .visible_cards(li, search)
            .into_iter()
            .find(|&i| i > current);
        if let Some(next) = next {
            self.board.selected_card[li] = next;
        }
    }

    /// Move selection up to the previous visible Card (previous match when
    /// a search is active).
    pub fn select_card_up(&mut self, search: Option<&str>) {
        let li = self.board.selected_list;
        let current = self.board.selected_card.get(li).copied().unwrap_or(0);
        let prev = self
            .board
            .visible_cards(li, search)
            .into_iter()
            .rev()
            .find(|&i| i < current);
        if let Some(prev) = prev {
            self.board.selected_card[li] = prev;
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
            if let Some(&ci) = self.board.visible_cards(li, Some(query)).first() {
                self.board.selected_list = li;
                self.board.selected_card[li] = ci;
                return;
            }
        }
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
    /// (computed by the renderer from the wrapped line count).
    pub fn scroll_detail(&mut self, step: usize, down: bool, max_scroll: usize) {
        if down {
            self.board.detail_scroll = (self.board.detail_scroll + step).min(max_scroll);
        } else {
            self.board.detail_scroll = self.board.detail_scroll.saturating_sub(step);
        }
    }


    /// Stage a card into the in-memory model without staging a write. Used
    /// when callers need to bring an archived card into scope before
    /// applying a command that touches it (e.g. `RestoreCard`).
    pub fn with_extra_card(&mut self, card: Card) {
        self.board.cards.insert(card.id.clone(), card);
    }

    /// Archived Cards of this board, read from disk.
    pub fn archived_cards(&self) -> Vec<Card> {
        card_store::list_archived_cards(&self.board.meta.id)
    }

    /// Archived Lists of this board, built from the board's `ListMeta`s and the
    /// in-memory Card map (Cards keep their `list_id` when their List is archived).
    pub fn archived_lists(&self) -> Vec<CardList> {
        crate::model::list::build_lists(&self.board.meta, &self.board.cards, true)
    }

    /// Permanently delete an archived Card's file. The only hard delete on
    /// Cards — everything else is Archive.
    pub fn delete_archived_card(&mut self, card_id: &ShortId) -> Result<(), BoardEditorError> {
        card_store::delete_card(&self.board.meta.id, card_id)?;
        self.board.cards.remove(card_id);
        Ok(())
    }

    /// Permanently delete an archived List along with its Cards: removes the
    /// Card files and the `ListMeta`, then persists the board.
    pub fn delete_archived_list(
        &mut self,
        list_id: &ShortId,
        card_ids: &[ShortId],
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
    pub fn last_added_card_id(&self) -> Option<&ShortId> {
        self.last_added_card_id.as_ref()
    }

    pub fn apply(&mut self, cmd: Command) -> Result<(), BoardEditorError> {
        self.last_added_card_id = None;
        let mut pending: Vec<PendingWrite> = Vec::new();
        match cmd {
            Command::ArchiveCard { card_id } => {
                // Membership (`list_id`/`position`) is left intact — only the
                // archived flag changes. visible_cards hides archived cards, and
                // restoring needs no membership reconstruction (no orphan window).
                let card = self.card_mut(&card_id)?;
                card.archived = true;
                card.log("Archived");
                pending.push(PendingWrite::Card(card.clone()));
            }
            Command::RestoreCard { card_id } => {
                // Reattach to a live List if the card's own list_id is missing
                // or points at a non-active List, so it can never come back
                // orphaned.
                let current_list_id = self.card_mut(&card_id)?.list_id.clone();
                let valid_home = self.board.lists.iter().any(|l| l.id == current_list_id);
                let reassign = if valid_home {
                    None
                } else if let Some(first) = self.board.lists.first() {
                    let pos = self.next_position(&first.id);
                    Some((first.id.clone(), pos))
                } else {
                    None
                };
                let card = self.card_mut(&card_id)?;
                card.archived = false;
                if let Some((lid, pos)) = reassign {
                    card.list_id = lid;
                    card.position = pos;
                }
                card.log("Restored from archive");
                let restored_list = card.list_id.clone();
                pending.push(PendingWrite::Card(card.clone()));
                // Reflect membership in the in-memory list ordering.
                if let Some(list) = self.board.lists.iter_mut().find(|l| l.id == restored_list)
                    && !list.card_ids.contains(&card_id)
                {
                    list.card_ids.push(card_id.clone());
                }
            }
            Command::EditCardTitle { card_id, title } => {
                let card = self.card_mut(&card_id)?;
                let changed = card.title != title;
                card.title = title;
                if changed {
                    card.log("Edited title");
                } else {
                    card.touch();
                }
                pending.push(PendingWrite::Card(card.clone()));
            }
            Command::EditCardDescription { card_id, body } => {
                let card = self.card_mut(&card_id)?;
                let changed = card.description != body;
                card.description = body;
                if changed {
                    card.log("Edited description");
                } else {
                    card.touch();
                }
                pending.push(PendingWrite::Card(card.clone()));
            }
            Command::SetDueDate { card_id, date } => {
                let card = self.card_mut(&card_id)?;
                let prev = card.due_date;
                card.due_date = Some(date);
                if prev != Some(date) {
                    card.log(format!("Set due date to {date}"));
                } else {
                    card.touch();
                }
                pending.push(PendingWrite::Card(card.clone()));
            }
            Command::ClearDueDate { card_id } => {
                let card = self.card_mut(&card_id)?;
                let was_set = card.due_date.is_some();
                card.due_date = None;
                if was_set {
                    card.log("Cleared due date");
                } else {
                    card.touch();
                }
                pending.push(PendingWrite::Card(card.clone()));
            }
            Command::AddChecklistItem { card_id, text } => {
                let card = self.card_mut(&card_id)?;
                let action = format!("Added checklist item '{text}'");
                card.checklist.push(ChecklistItem { text, completed: false });
                card.log(action);
                let last = card.checklist.len() - 1;
                pending.push(PendingWrite::Card(card.clone()));
                // Detail cursor follows the new item.
                self.board.detail_item_idx = last;
            }
            Command::EditChecklistItem { card_id, item_idx, text } => {
                let card = self.card_mut(&card_id)?;
                let item = card
                    .checklist
                    .get_mut(item_idx)
                    .ok_or(BoardEditorError::Invariant("checklist index out of range"))?;
                let changed = item.text != text;
                let old = std::mem::replace(&mut item.text, text);
                let new = item.text.clone();
                if changed {
                    card.log(format!("Renamed checklist item '{old}' → '{new}'"));
                } else {
                    card.touch();
                }
                pending.push(PendingWrite::Card(card.clone()));
            }
            Command::ToggleChecklistItem { card_id, item_idx } => {
                let card = self.card_mut(&card_id)?;
                let item = card
                    .checklist
                    .get_mut(item_idx)
                    .ok_or(BoardEditorError::Invariant("checklist index out of range"))?;
                item.completed = !item.completed;
                let action = if item.completed {
                    format!("Completed checklist item '{}'", item.text)
                } else {
                    format!("Uncompleted checklist item '{}'", item.text)
                };
                card.log(action);
                pending.push(PendingWrite::Card(card.clone()));
            }
            Command::RemoveChecklistItem { card_id, item_idx } => {
                let card = self.card_mut(&card_id)?;
                if item_idx >= card.checklist.len() {
                    return Err(BoardEditorError::Invariant(
                        "checklist index out of range",
                    ));
                }
                let removed = card.checklist.remove(item_idx);
                card.log(format!("Removed checklist item '{}'", removed.text));
                let len = card.checklist.len();
                pending.push(PendingWrite::Card(card.clone()));
                // Clamp detail cursor to the shrunk checklist.
                if self.board.detail_item_idx >= len && len > 0 {
                    self.board.detail_item_idx = len - 1;
                }
            }
            Command::ReorderChecklistItem { card_id, from, to } => {
                let card = self.card_mut(&card_id)?;
                let len = card.checklist.len();
                if from >= len || to >= len {
                    return Err(BoardEditorError::Invariant(
                        "checklist index out of range",
                    ));
                }
                if from != to {
                    let name = card.checklist[from].text.clone();
                    let item = card.checklist.remove(from);
                    card.checklist.insert(to, item);
                    card.log(format!("Reordered checklist item '{name}'"));
                } else {
                    card.touch();
                }
                pending.push(PendingWrite::Card(card.clone()));
                // Detail cursor follows the moved item.
                self.board.detail_item_idx = to;
            }
            Command::ToggleLabel { card_id, label_id } => {
                let label_name = self
                    .board
                    .meta
                    .labels
                    .iter()
                    .find(|l| l.id == label_id)
                    .map(|l| l.name.clone())
                    .ok_or_else(|| BoardEditorError::NotFound {
                        kind: "label",
                        id: label_id.clone(),
                    })?;
                let card = self.card_mut(&card_id)?;
                let action = if let Some(pos) = card.label_ids.iter().position(|id| *id == label_id) {
                    card.label_ids.remove(pos);
                    format!("Removed label '{label_name}'")
                } else {
                    card.label_ids.push(label_id);
                    format!("Added label '{label_name}'")
                };
                card.log(action);
                pending.push(PendingWrite::Card(card.clone()));
            }
            Command::AddCard { list_id, title } => {
                let li = self
                    .board
                    .lists
                    .iter()
                    .position(|l| l.id == list_id)
                    .ok_or_else(|| BoardEditorError::NotFound {
                        kind: "list",
                        id: list_id.clone(),
                    })?;
                let pos = self.next_position(&list_id);
                let mut card = Card::new(title);
                card.list_id = list_id.clone();
                card.position = pos;
                card.log("Created");
                let new_id = card.id.clone();
                let list = &mut self.board.lists[li];
                list.card_ids.push(card.id.clone());
                let new_idx = list.card_ids.len() - 1;
                self.board.cards.insert(card.id.clone(), card.clone());
                pending.push(PendingWrite::Card(card));
                self.last_added_card_id = Some(new_id);
                // Selection follows the new card.
                self.board.selected_list = li;
                if let Some(slot) = self.board.selected_card.get_mut(li) {
                    *slot = new_idx;
                }
            }
            Command::MoveCard { card_id, direction } => {
                self.move_card(&card_id, direction, &mut pending)?;
            }
            Command::AddList { name } => {
                let meta = ListMeta { id: crate::model::ids::new_id(), name, archived: false };
                self.board.lists.push(CardList {
                    id: meta.id.clone(),
                    name: meta.name.clone(),
                    card_ids: Vec::new(),
                    archived: false,
                });
                self.board.meta.lists.push(meta);
                self.board.selected_card.push(0);
                self.board.scroll_offset.push(0);
                // Selection follows the new list.
                self.board.selected_list = self.board.lists.len() - 1;
                pending.push(PendingWrite::Board);
            }
            Command::ArchiveList { list_id } => {
                let pos = self
                    .board
                    .lists
                    .iter()
                    .position(|l| l.id == list_id)
                    .ok_or_else(|| BoardEditorError::NotFound {
                        kind: "list",
                        id: list_id.clone(),
                    })?;
                // Flip the ListMeta flag; Cards keep their list_id and reappear
                // on restore. Drop the active-list view row.
                if let Some(lm) = self.board.meta.lists.iter_mut().find(|l| l.id == list_id) {
                    lm.archived = true;
                }
                self.board.lists.remove(pos);
                self.board.selected_card.remove(pos);
                self.board.scroll_offset.remove(pos);
                if self.board.selected_list > 0
                    && self.board.selected_list >= self.board.lists.len()
                {
                    self.board.selected_list = self.board.lists.len().saturating_sub(1);
                }
                pending.push(PendingWrite::Board);
            }
            Command::RestoreList { list_id } => {
                let lm = self
                    .board
                    .meta
                    .lists
                    .iter_mut()
                    .find(|l| l.id == list_id)
                    .ok_or_else(|| BoardEditorError::NotFound {
                        kind: "list",
                        id: list_id.clone(),
                    })?;
                lm.archived = false;
                let name = lm.name.clone();
                // Cards were never moved; rebuild the active-list row from them.
                let card_ids = crate::model::list::ordered_card_ids(&list_id, &self.board.cards);
                self.board.lists.push(CardList {
                    id: list_id.clone(),
                    name,
                    card_ids,
                    archived: false,
                });
                self.board.selected_card.push(0);
                self.board.scroll_offset.push(0);
                pending.push(PendingWrite::Board);
            }
            Command::RenameList { list_id, name } => {
                let lm = self
                    .board
                    .meta
                    .lists
                    .iter_mut()
                    .find(|l| l.id == list_id)
                    .ok_or_else(|| BoardEditorError::NotFound {
                        kind: "list",
                        id: list_id.clone(),
                    })?;
                lm.name = name.clone();
                if let Some(list) = self.board.lists.iter_mut().find(|l| l.id == list_id) {
                    list.name = name;
                }
                pending.push(PendingWrite::Board);
            }
            Command::MoveList { list_id, direction } => {
                self.move_list(&list_id, direction, &mut pending)?;
            }
            Command::ArchiveBoard { board_id } => {
                if board_id != self.board.meta.id {
                    return Err(BoardEditorError::NotFound {
                        kind: "board",
                        id: board_id,
                    });
                }
                self.board.meta.archived = true;
                pending.push(PendingWrite::Board);
            }
            Command::RestoreBoard { board_id } => {
                if board_id != self.board.meta.id {
                    return Err(BoardEditorError::NotFound {
                        kind: "board",
                        id: board_id,
                    });
                }
                self.board.meta.archived = false;
                pending.push(PendingWrite::Board);
            }
            Command::RenameBoard { name } => {
                self.board.meta.name = name;
                pending.push(PendingWrite::Board);
            }
            Command::SetAccentColor { color } => {
                self.board.meta.accent_color = color;
                pending.push(PendingWrite::Board);
            }
            Command::DefineLabel { name, color } => {
                let label = Label::new(name, color);
                self.board.meta.labels.push(label);
                pending.push(PendingWrite::Board);
            }
            Command::RenameLabel { label_id, name } => {
                let label = self
                    .board
                    .meta
                    .labels
                    .iter_mut()
                    .find(|l| l.id == label_id)
                    .ok_or_else(|| BoardEditorError::NotFound {
                        kind: "label",
                        id: label_id.clone(),
                    })?;
                label.name = name;
                pending.push(PendingWrite::Board);
            }
            Command::SetLabelColor { label_id, color } => {
                let label = self
                    .board
                    .meta
                    .labels
                    .iter_mut()
                    .find(|l| l.id == label_id)
                    .ok_or_else(|| BoardEditorError::NotFound {
                        kind: "label",
                        id: label_id.clone(),
                    })?;
                label.color = color;
                pending.push(PendingWrite::Board);
            }
            Command::DeleteLabel { label_id } => {
                let pos = self
                    .board
                    .meta
                    .labels
                    .iter()
                    .position(|l| l.id == label_id)
                    .ok_or_else(|| BoardEditorError::NotFound {
                        kind: "label",
                        id: label_id.clone(),
                    })?;
                self.board.meta.labels.remove(pos);
                // Dead label_ids are NOT scrubbed from every card here. They are
                // harmless in memory (resolved_labels/matches_search ignore
                // unknown ids) and get pruned lazily the next time each card is
                // saved (see commit_pending).
                pending.push(PendingWrite::Board);
            }
            Command::ReorderLabels { from, to } => {
                let len = self.board.meta.labels.len();
                if from >= len || to >= len {
                    return Err(BoardEditorError::Invariant("label index out of range"));
                }
                if from != to {
                    self.board.meta.labels.swap(from, to);
                    pending.push(PendingWrite::Board);
                }
            }
        }
        self.commit_pending(pending)?;
        self.board.clamp_selection();
        Ok(())
    }

    /// Fractional rank one past the last Card currently in `list_id`.
    fn next_position(&self, list_id: &str) -> f64 {
        self.board
            .cards
            .values()
            .filter(|c| c.list_id == list_id)
            .map(|c| c.position)
            .fold(0.0_f64, f64::max)
            + 1.0
    }

    /// Position of the Card at `idx` in `card_ids`, or None if out of range.
    fn position_at(&self, card_ids: &[ShortId], idx: usize) -> Option<f64> {
        card_ids.get(idx).and_then(|id| self.board.cards.get(id)).map(|c| c.position)
    }

    fn card_mut(&mut self, card_id: &ShortId) -> Result<&mut Card, BoardEditorError> {
        self.board
            .cards
            .get_mut(card_id)
            .ok_or_else(|| BoardEditorError::NotFound {
                kind: "card",
                id: card_id.clone(),
            })
    }

    fn move_card(
        &mut self,
        card_id: &ShortId,
        direction: MoveDir,
        pending: &mut Vec<PendingWrite>,
    ) -> Result<(), BoardEditorError> {
        // Locate the list and position
        let (src_idx, ci) = self
            .board
            .lists
            .iter()
            .enumerate()
            .find_map(|(li, l)| l.card_ids.iter().position(|id| id == card_id).map(|ci| (li, ci)))
            .ok_or_else(|| BoardEditorError::NotFound {
                kind: "card",
                id: card_id.clone(),
            })?;

        // Within-list reorder swaps card_ids and re-ranks only the moved card;
        // cross-list moves also rewrite the card's list_id. Either way exactly
        // one Card file is written.
        match direction {
            MoveDir::Up => {
                if ci > 0 {
                    self.board.lists[src_idx].card_ids.swap(ci, ci - 1);
                    let moved = self.reposition(src_idx, ci - 1);
                    if let Some(slot) = self.board.selected_card.get_mut(src_idx) {
                        *slot = ci - 1;
                    }
                    pending.push(PendingWrite::Card(self.board.cards[&moved].clone()));
                }
            }
            MoveDir::Down => {
                let len = self.board.lists[src_idx].card_ids.len();
                if ci + 1 < len {
                    self.board.lists[src_idx].card_ids.swap(ci, ci + 1);
                    let moved = self.reposition(src_idx, ci + 1);
                    if let Some(slot) = self.board.selected_card.get_mut(src_idx) {
                        *slot = ci + 1;
                    }
                    pending.push(PendingWrite::Card(self.board.cards[&moved].clone()));
                }
            }
            MoveDir::Left | MoveDir::Right => {
                let dst = match direction {
                    MoveDir::Left if src_idx > 0 => src_idx - 1,
                    MoveDir::Right if src_idx + 1 < self.board.lists.len() => src_idx + 1,
                    _ => return Ok(()),
                };
                let cid = self.board.lists[src_idx].card_ids.remove(ci);
                let insert_at = ci.min(self.board.lists[dst].card_ids.len());
                self.board.lists[dst].card_ids.insert(insert_at, cid.clone());
                let dst_list_id = self.board.lists[dst].id.clone();
                if let Some(card) = self.board.cards.get_mut(&cid) {
                    card.list_id = dst_list_id;
                }
                self.reposition(dst, insert_at);
                self.board.selected_list = dst;
                if let Some(slot) = self.board.selected_card.get_mut(dst) {
                    *slot = insert_at;
                }
                pending.push(PendingWrite::Card(self.board.cards[&cid].clone()));
            }
        }
        Ok(())
    }

    /// Re-rank the Card at `idx` in list `list_idx` to sit strictly between its
    /// new neighbors' positions. Returns the moved Card's id.
    fn reposition(&mut self, list_idx: usize, idx: usize) -> ShortId {
        let card_ids = self.board.lists[list_idx].card_ids.clone();
        let prev = if idx > 0 { self.position_at(&card_ids, idx - 1) } else { None };
        let next = self.position_at(&card_ids, idx + 1);
        let new_pos = crate::model::list::fractional_between(prev, next);
        let id = card_ids[idx].clone();
        if let Some(card) = self.board.cards.get_mut(&id) {
            card.position = new_pos;
        }
        id
    }

    fn move_list(
        &mut self,
        list_id: &ShortId,
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
                id: list_id.clone(),
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

    fn commit_pending(&self, writes: Vec<PendingWrite>) -> Result<(), BoardEditorError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::board::BoardMeta;
    use crate::model::ids;
    use crate::test_support::with_temp_dir;
    use std::collections::HashMap;

    fn seed_board_with_card() -> (BoardMeta, String) {
        let mut meta = BoardMeta::new("Test".into());
        let list_id = ids::new_id();
        meta.lists.push(ListMeta { id: list_id.clone(), name: "Backlog".into(), archived: false });
        let mut card = Card::new("Task".into());
        card.list_id = list_id;
        card.position = 1.0;
        board_store::save_board(&meta).unwrap();
        card_store::save_card(&meta.id, &card).unwrap();
        (meta, card.id)
    }

    #[test]
    fn apply_archive_card_persists_and_logs() {
        with_temp_dir(|| {
            let (meta, card_id) = seed_board_with_card();
            let mut editor = BoardEditor::load(&meta.id).unwrap();
            editor.apply(Command::ArchiveCard { card_id: card_id.clone() }).unwrap();

            let card = editor.board().cards.get(&card_id).unwrap();
            assert!(card.archived);
            assert!(card.history.iter().any(|h| h.action == "Archived"));

            let reloaded = card_store::load_card(&meta.id, &card_id).unwrap();
            assert!(reloaded.archived);
            assert!(reloaded.history.iter().any(|h| h.action == "Archived"));
        });
    }

    #[test]
    fn apply_restore_card_clears_archive_and_logs() {
        with_temp_dir(|| {
            let (meta, card_id) = seed_board_with_card();
            let mut editor = BoardEditor::load(&meta.id).unwrap();
            editor.apply(Command::ArchiveCard { card_id: card_id.clone() }).unwrap();
            editor.apply(Command::RestoreCard { card_id: card_id.clone() }).unwrap();

            let card = editor.board().cards.get(&card_id).unwrap();
            assert!(!card.archived);
            let actions: Vec<&str> = card.history.iter().map(|h| h.action.as_str()).collect();
            assert!(actions.contains(&"Archived"));
            assert!(actions.contains(&"Restored from archive"));
        });
    }

    fn fixed_card(id: &str, title: &str) -> Card {
        let mut c = Card::new(title.into());
        c.id = id.into();
        c
    }

    fn loaded_board(cards: Vec<Card>) -> LoadedBoard {
        let card_ids: Vec<_> = cards.iter().map(|c| c.id.clone()).collect();
        let cards_map: HashMap<_, _> = cards.into_iter().map(|c| (c.id.clone(), c)).collect();
        LoadedBoard {
            meta: BoardMeta::new("X".into()),
            lists: vec![CardList { id: "l".into(), name: "L".into(), card_ids, archived: false }],
            cards: cards_map,
            selected_list: 0,
            selected_card: vec![0],
            scroll_offset: vec![0],
            detail_item_idx: 0,
            detail_scroll: 0,
        }
    }

    #[test]
    fn visible_cards_empty_for_no_search_match() {
        let board = loaded_board(vec![fixed_card("c1", "alpha")]);
        assert!(board.visible_cards(0, Some("zzz")).is_empty());
    }

    #[test]
    fn visible_cards_skips_archived() {
        let mut cards = vec![fixed_card("id1", "a"), fixed_card("id2", "b")];
        cards[1].archived = true;
        let board = loaded_board(cards);
        assert_eq!(board.visible_cards(0, None), vec![0]);
    }

    #[test]
    fn visible_cards_search_hides_non_matching() {
        let cards = vec![
            fixed_card("c1", "alpha"),
            fixed_card("c2", "BINGO"),
            fixed_card("c3", "gamma"),
        ];
        let board = loaded_board(cards);
        assert_eq!(board.visible_cards(0, Some("bingo")), vec![1]);
        assert_eq!(board.visible_cards(0, None), vec![0, 1, 2]);
    }

    #[test]
    fn select_card_down_skips_to_next_search_match() {
        let cards = vec![
            fixed_card("c1", "alpha"),
            fixed_card("c2", "BINGO match"),
            fixed_card("c3", "gamma"),
        ];
        let mut editor = BoardEditor::from_loaded(loaded_board(cards));
        editor.select_card_down(Some("BINGO"));
        assert_eq!(editor.board().selected_card[0], 1);
        editor.select_card_down(Some("BINGO"));
        assert_eq!(editor.board().selected_card[0], 1);
    }

    #[test]
    fn apply_missing_card_returns_not_found() {
        with_temp_dir(|| {
            let (meta, _) = seed_board_with_card();
            let mut editor = BoardEditor::load(&meta.id).unwrap();
            let bogus = ids::new_id();
            let err = editor.apply(Command::ArchiveCard { card_id: bogus }).unwrap_err();
            assert!(matches!(err, BoardEditorError::NotFound { kind: "card", .. }));
        });
    }
}
