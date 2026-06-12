//! Board Editor — aggregate root for one Loaded Board.
//!
//! See `docs/adr/0001-board-editor-aggregate.md`. Owns the in-memory board
//! state; mutations enter via `apply(Command)` which mutates, appends the
//! History Entry, and stages writes for `commit_pending`. Selection State
//! is mutated only through the selection verbs below (ADR-0002).

use std::collections::HashMap;

use crate::app::LoadedBoard;
use crate::command::{Command, MoveDir};
use crate::model::card::{Card, ChecklistItem};
use crate::model::ids::ShortId;
use crate::model::label::Label;
use crate::model::list::CardList;
use crate::storage::{self, board_store, card_store, list_store};

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
    List(CardList),
    Board,
}

impl BoardEditor {
    pub fn load(board_id: &str) -> Result<Self, BoardEditorError> {
        let meta = board_store::load_board(board_id)?;
        let lists = list_store::load_all_lists(board_id, &meta.list_order)?;
        let mut cards = HashMap::new();
        for list in &lists {
            for card_id in &list.card_ids {
                if let Ok(card) = card_store::load_card(board_id, card_id) {
                    cards.insert(card_id.clone(), card);
                }
            }
        }
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

    /// Re-read the board from disk, preserving Selection State.
    /// `Err(NotFound)`-style storage errors on the board file mean the board
    /// is gone; list-load failures keep the in-memory state untouched.
    pub fn reload(&mut self) -> Result<(), BoardEditorError> {
        let meta = board_store::load_board(&self.board.meta.id)?;
        let lists = match list_store::load_all_lists(&self.board.meta.id, &meta.list_order) {
            Ok(l) => l,
            Err(_) => return Ok(()),
        };
        let mut cards = HashMap::new();
        for list in &lists {
            for card_id in &list.card_ids {
                if let Ok(card) = card_store::load_card(&self.board.meta.id, card_id) {
                    cards.insert(card_id.clone(), card);
                }
            }
        }

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

    /// Archived Lists of this board, read from disk.
    pub fn archived_lists(&self) -> Vec<CardList> {
        list_store::list_archived_lists(&self.board.meta.id)
    }

    /// Permanently delete an archived Card's file. The only hard delete on
    /// Cards — everything else is Archive.
    pub fn delete_archived_card(&mut self, card_id: &ShortId) -> Result<(), BoardEditorError> {
        card_store::delete_card(&self.board.meta.id, card_id)?;
        self.board.cards.remove(card_id);
        Ok(())
    }

    /// Permanently delete an archived List's file along with its Cards.
    pub fn delete_archived_list(
        &mut self,
        list_id: &ShortId,
        card_ids: &[ShortId],
    ) -> Result<(), BoardEditorError> {
        for cid in card_ids {
            let _ = card_store::delete_card(&self.board.meta.id, cid);
            self.board.cards.remove(cid);
        }
        list_store::delete_list_file(&self.board.meta.id, list_id)?;
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
                let card = self.card_mut(&card_id)?;
                card.archived = true;
                card.log("Archived");
                pending.push(PendingWrite::Card(card.clone()));
                // Remove from list ids
                let cid = card_id.clone();
                for list in &mut self.board.lists {
                    if list.card_ids.iter().any(|id| id == &cid) {
                        list.card_ids.retain(|id| id != &cid);
                        pending.push(PendingWrite::List(list.clone()));
                    }
                }
            }
            Command::RestoreCard { card_id } => {
                let card = self.card_mut(&card_id)?;
                card.archived = false;
                card.log("Restored from archive");
                pending.push(PendingWrite::Card(card.clone()));
                // Append to selected list if not already on any list
                let cid = card_id.clone();
                let already_on_a_list = self
                    .board
                    .lists
                    .iter()
                    .any(|l| l.card_ids.iter().any(|id| id == &cid));
                if !already_on_a_list {
                    let li = self.board.selected_list.min(self.board.lists.len().saturating_sub(1));
                    if let Some(target) = self.board.lists.get_mut(li) {
                        target.card_ids.push(cid);
                        pending.push(PendingWrite::List(target.clone()));
                    }
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
                let list = &mut self.board.lists[li];
                let mut card = Card::new(title);
                card.log("Created");
                list.card_ids.push(card.id.clone());
                let new_id = card.id.clone();
                let new_idx = list.card_ids.len() - 1;
                pending.push(PendingWrite::List(list.clone()));
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
                let list = CardList::new(name);
                let lid = list.id.clone();
                self.board.meta.list_order.push(lid.clone());
                self.board.lists.push(list.clone());
                self.board.selected_card.push(0);
                self.board.scroll_offset.push(0);
                // Selection follows the new list.
                self.board.selected_list = self.board.lists.len() - 1;
                pending.push(PendingWrite::List(list));
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
                let mut list = self.board.lists.remove(pos);
                list.archived = true;
                self.board.selected_card.remove(pos);
                self.board.scroll_offset.remove(pos);
                self.board.meta.list_order.retain(|id| id != &list_id);
                if self.board.selected_list > 0
                    && self.board.selected_list >= self.board.lists.len()
                {
                    self.board.selected_list = self.board.lists.len().saturating_sub(1);
                }
                pending.push(PendingWrite::List(list));
                pending.push(PendingWrite::Board);
            }
            Command::RestoreList { list_id } => {
                let mut list = list_store::load_list(&self.board.meta.id, &list_id)
                    .map_err(|_| BoardEditorError::NotFound {
                        kind: "list",
                        id: list_id.clone(),
                    })?;
                list.archived = false;
                if !self.board.meta.list_order.contains(&list.id) {
                    self.board.meta.list_order.push(list.id.clone());
                }
                // Reload cards for restored list
                for card_id in &list.card_ids {
                    if let Ok(card) = card_store::load_card(&self.board.meta.id, card_id) {
                        self.board.cards.insert(card_id.clone(), card);
                    }
                }
                self.board.lists.push(list.clone());
                self.board.selected_card.push(0);
                self.board.scroll_offset.push(0);
                pending.push(PendingWrite::List(list));
                pending.push(PendingWrite::Board);
            }
            Command::RenameList { list_id, name } => {
                let list = self
                    .board
                    .lists
                    .iter_mut()
                    .find(|l| l.id == list_id)
                    .ok_or_else(|| BoardEditorError::NotFound {
                        kind: "list",
                        id: list_id.clone(),
                    })?;
                list.name = name;
                pending.push(PendingWrite::List(list.clone()));
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
                for card in self.board.cards.values_mut() {
                    if card.label_ids.iter().any(|id| id == &label_id) {
                        card.label_ids.retain(|id| id != &label_id);
                        pending.push(PendingWrite::Card(card.clone()));
                    }
                }
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

        match direction {
            MoveDir::Up => {
                if ci > 0 {
                    let list = &mut self.board.lists[src_idx];
                    list.card_ids.swap(ci, ci - 1);
                    if let Some(slot) = self.board.selected_card.get_mut(src_idx) {
                        *slot = ci - 1;
                    }
                    pending.push(PendingWrite::List(list.clone()));
                }
            }
            MoveDir::Down => {
                let len = self.board.lists[src_idx].card_ids.len();
                if ci + 1 < len {
                    let list = &mut self.board.lists[src_idx];
                    list.card_ids.swap(ci, ci + 1);
                    if let Some(slot) = self.board.selected_card.get_mut(src_idx) {
                        *slot = ci + 1;
                    }
                    pending.push(PendingWrite::List(list.clone()));
                }
            }
            MoveDir::Left => {
                if src_idx == 0 {
                    return Ok(());
                }
                let dst = src_idx - 1;
                let cid = self.board.lists[src_idx].card_ids.remove(ci);
                let src_clone = self.board.lists[src_idx].clone();
                pending.push(PendingWrite::List(src_clone));
                let insert_at = ci.min(self.board.lists[dst].card_ids.len());
                self.board.lists[dst].card_ids.insert(insert_at, cid);
                pending.push(PendingWrite::List(self.board.lists[dst].clone()));
                self.board.selected_list = dst;
                if let Some(slot) = self.board.selected_card.get_mut(dst) {
                    *slot = insert_at;
                }
            }
            MoveDir::Right => {
                if src_idx + 1 >= self.board.lists.len() {
                    return Ok(());
                }
                let dst = src_idx + 1;
                let cid = self.board.lists[src_idx].card_ids.remove(ci);
                let src_clone = self.board.lists[src_idx].clone();
                pending.push(PendingWrite::List(src_clone));
                let insert_at = ci.min(self.board.lists[dst].card_ids.len());
                self.board.lists[dst].card_ids.insert(insert_at, cid);
                pending.push(PendingWrite::List(self.board.lists[dst].clone()));
                self.board.selected_list = dst;
                if let Some(slot) = self.board.selected_card.get_mut(dst) {
                    *slot = insert_at;
                }
            }
        }
        Ok(())
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
        self.board.lists.swap(i, target);
        self.board.meta.list_order.swap(i, target);
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
                PendingWrite::Card(c) => card_store::save_card(&self.board.meta.id, &c)?,
                PendingWrite::List(l) => list_store::save_list(&self.board.meta.id, &l)?,
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

    fn seed_board_with_card() -> (BoardMeta, String) {
        let mut meta = BoardMeta::new("Test".into());
        let mut list = CardList::new("Backlog".into());
        let card = Card::new("Task".into());
        list.card_ids.push(card.id.clone());
        meta.list_order.push(list.id.clone());
        board_store::save_board(&meta).unwrap();
        list_store::save_list(&meta.id, &list).unwrap();
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
