//! Board Editor — aggregate root for one Loaded Board.
//!
//! See `docs/adr/0001-board-editor-aggregate.md`. Owns the in-memory board
//! state; mutations enter via `apply(Command)` which mutates, appends the
//! History Entry, and stages writes for `commit_pending`. Selection State
//! is mutated only through the selection verbs below (ADR-0002).

use std::collections::{HashMap, HashSet};

use crate::app::LoadedBoard;
use crate::command::{Command, MoveDir};
use crate::model::board::{BoardMeta, ListMeta};
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
    /// Non-fatal issues found and auto-repaired while loading the board (orphan
    /// cards reattached, dangling label refs stripped). Surfaced to the user as
    /// a status message; empty on a clean load.
    pub diagnostics: Vec<String>,
}

enum PendingWrite {
    Card(Card),
    Board,
}

/// Detect and auto-repair dangling references in freshly loaded cards, persisting
/// each card that changed. Returns a human-readable line per repair (empty on a
/// clean board). Two reference hazards are handled:
///
/// * **Orphan card** — `list_id` names a List that no longer exists (or is
///   empty). The card would be invisible (never built into any List), so it is
///   reattached to the board's first active List with a position past the
///   current tail.
/// * **Dangling label** — `label_ids` names a Label absent from the board; the
///   id is stripped.
fn repair_references(
    board_id: &str,
    meta: &BoardMeta,
    cards: &mut HashMap<ShortId, Card>,
) -> Vec<String> {
    let valid_lists: HashSet<&ShortId> = meta.lists.iter().map(|l| &l.id).collect();
    let valid_labels: HashSet<&ShortId> = meta.labels.iter().map(|l| &l.id).collect();
    let default_list = meta
        .lists
        .iter()
        .find(|l| !l.archived)
        .or_else(|| meta.lists.first())
        .map(|l| l.id.clone());

    // Next free position in the reattach target, so reattached orphans land at
    // the tail rather than colliding at 0.
    let mut next_pos = default_list
        .as_ref()
        .map(|dst| {
            cards
                .values()
                .filter(|c| &c.list_id == dst)
                .map(|c| c.position)
                .fold(0.0_f64, f64::max)
                + 1.0
        })
        .unwrap_or(1.0);

    let mut warnings = Vec::new();
    let mut changed: Vec<ShortId> = Vec::new();

    for card in cards.values_mut() {
        let mut card_changed = false;

        if !valid_lists.contains(&card.list_id) {
            match &default_list {
                Some(dst) => {
                    warnings.push(format!(
                        "card \"{}\" pointed at missing list — reattached to first list",
                        card.title
                    ));
                    card.list_id = dst.clone();
                    card.position = next_pos;
                    next_pos += 1.0;
                    card_changed = true;
                }
                None => warnings.push(format!(
                    "card \"{}\" is orphaned and the board has no lists to attach it to",
                    card.title
                )),
            }
        }

        let before = card.label_ids.len();
        card.label_ids.retain(|lid| valid_labels.contains(lid));
        let stripped = before - card.label_ids.len();
        if stripped > 0 {
            warnings.push(format!(
                "card \"{}\" had {stripped} unknown label ref(s) removed",
                card.title
            ));
            card_changed = true;
        }

        if card_changed {
            changed.push(card.id.clone());
        }
    }

    for id in &changed {
        if let Some(card) = cards.get(id) {
            let _ = card_store::save_card(board_id, card);
        }
    }

    warnings
}

impl BoardEditor {
    pub fn load(board_id: &str) -> Result<Self, BoardEditorError> {
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
                scroll_offset: vec![0; num_lists],
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
        Self { board, last_added_card_id: None, diagnostics: Vec::new() }
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
        if let Some(&next_raw) = self.board.visible_cards(li, search).iter().find(|&&i| i > cur_raw)
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
        if let Some(&prev_raw) =
            self.board.visible_cards(li, search).iter().rev().find(|&&i| i < cur_raw)
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
                if let Some(ord) = self.board.visible_cards(li, None).iter().position(|&r| r == raw)
                {
                    self.board.selected_card[li] = ord;
                }
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
                self.board.lists[li].card_ids.push(card.id.clone());
                self.board.cards.insert(card.id.clone(), card.clone());
                pending.push(PendingWrite::Card(card));
                self.last_added_card_id = Some(new_id);
                // Selection follows the new card — it is non-archived and last
                // in the list, so its visible ordinal is the last visible slot.
                self.board.selected_list = li;
                let ord = self.board.visible_cards(li, None).len().saturating_sub(1);
                if let Some(slot) = self.board.selected_card.get_mut(li) {
                    *slot = ord;
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
        #[cfg(debug_assertions)]
        if let Err(e) = self.check_invariants() {
            panic!("board invariant violated after applying command: {e}");
        }
        Ok(())
    }

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
    fn check_invariants(&self) -> Result<(), String> {
        let b = &self.board;
        let valid_lists: HashSet<&ShortId> = b.meta.lists.iter().map(|l| &l.id).collect();

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
            return Err(format!("selected_list {} out of range (lists: {n})", b.selected_list));
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

    /// `position` of the Card at raw `card_ids` index `idx` in list `list_idx`.
    fn pos_of_raw(&self, list_idx: usize, idx: usize) -> Option<f64> {
        self.board.lists[list_idx]
            .card_ids
            .get(idx)
            .and_then(|id| self.board.cards.get(id))
            .map(|c| c.position)
    }

    /// Ordinal of `card_id` within list `list_idx`'s visible (non-archived)
    /// sequence — the index space of `selected_card`.
    fn visible_ordinal(&self, list_idx: usize, card_id: &ShortId) -> Option<usize> {
        let card_ids = &self.board.lists[list_idx].card_ids;
        self.board
            .visible_cards(list_idx, None)
            .into_iter()
            .position(|raw| card_ids[raw] == *card_id)
    }

    /// Re-derive a list's in-memory `card_ids` from the current Card positions,
    /// so the live view matches what a reload would produce.
    fn rebuild_list(&mut self, list_idx: usize) {
        let lid = self.board.lists[list_idx].id.clone();
        self.board.lists[list_idx].card_ids =
            crate::model::list::ordered_card_ids(&lid, &self.board.cards);
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
                        let prev = (vp >= 2).then(|| self.pos_of_raw(src_idx, vis[vp - 2])).flatten();
                        let next = self.pos_of_raw(src_idx, vis[vp - 1]);
                        crate::model::list::fractional_between(prev, next)
                    }
                    MoveDir::Down => {
                        if vp + 1 >= vis.len() {
                            return Ok(());
                        }
                        let prev = self.pos_of_raw(src_idx, vis[vp + 1]);
                        let next = vis.get(vp + 2).and_then(|&r| self.pos_of_raw(src_idx, r));
                        crate::model::list::fractional_between(prev, next)
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
                pending.push(PendingWrite::Card(self.board.cards[card_id].clone()));
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
                let next = vis_dst.get(insert_vp).and_then(|&r| self.pos_of_raw(dst, r));
                let new_pos = crate::model::list::fractional_between(prev, next);
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
                pending.push(PendingWrite::Card(self.board.cards[card_id].clone()));
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
    fn load_reattaches_orphan_card_and_reports() {
        with_temp_dir(|| {
            let (meta, _) = seed_board_with_card();
            let list_id = meta.lists[0].id.clone();
            // A card whose list_id names a list that does not exist.
            let mut orphan = Card::new("Lost".into());
            orphan.list_id = "ghostlst".into();
            card_store::save_card(&meta.id, &orphan).unwrap();

            let editor = BoardEditor::load(&meta.id).unwrap();
            assert_eq!(editor.diagnostics.len(), 1);
            // Reattached to the only (first) list, in memory and on disk.
            assert_eq!(editor.board().cards[&orphan.id].list_id, list_id);
            let reloaded = card_store::load_card(&meta.id, &orphan.id).unwrap();
            assert_eq!(reloaded.list_id, list_id);
            // And now visible in that list.
            assert!(editor.board().lists[0].card_ids.contains(&orphan.id));
        });
    }

    #[test]
    fn load_strips_dangling_label_refs() {
        with_temp_dir(|| {
            let (meta, card_id) = seed_board_with_card();
            let mut card = card_store::load_card(&meta.id, &card_id).unwrap();
            card.label_ids = vec!["nolabel1".into(), "nolabel2".into()];
            card_store::save_card(&meta.id, &card).unwrap();

            let editor = BoardEditor::load(&meta.id).unwrap();
            assert_eq!(editor.diagnostics.len(), 1);
            assert!(editor.board().cards[&card_id].label_ids.is_empty());
            let reloaded = card_store::load_card(&meta.id, &card_id).unwrap();
            assert!(reloaded.label_ids.is_empty());
        });
    }

    #[test]
    fn load_clean_board_has_no_diagnostics() {
        with_temp_dir(|| {
            let (meta, _) = seed_board_with_card();
            let editor = BoardEditor::load(&meta.id).unwrap();
            assert!(editor.diagnostics.is_empty());
        });
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

    /// Regression: a visible Card with archived (hidden) Cards interleaved
    /// between it and its visible neighbor must still move one visible slot.
    /// Before the fix, the move swapped raw `card_ids` slots and merely traded
    /// places with a hidden Card — no visible change.
    #[test]
    fn move_up_skips_interleaved_archived_card() {
        with_temp_dir(|| {
            let mut meta = BoardMeta::new("Test".into());
            let lid = ids::new_id();
            meta.lists.push(ListMeta { id: lid.clone(), name: "TODO".into(), archived: false });
            board_store::save_board(&meta).unwrap();

            // Visible "top" (pos -1), archived (0, 1), visible "bottom" (pos 2).
            let mk = |title: &str, pos: f64, archived: bool| {
                let mut c = Card::new(title.into());
                c.list_id = lid.clone();
                c.position = pos;
                c.archived = archived;
                card_store::save_card(&meta.id, &c).unwrap();
                c.id
            };
            let top = mk("top", -1.0, false);
            let _a1 = mk("arch1", 0.0, true);
            let _a2 = mk("arch2", 1.0, true);
            let bottom = mk("bottom", 2.0, false);

            let mut editor = BoardEditor::load(&meta.id).unwrap();
            // Visible order is [top, bottom]; bottom sits at raw index 3.
            let vis = editor.board().visible_cards(0, None);
            assert_eq!(vis.len(), 2);

            editor.apply(Command::MoveCard { card_id: bottom.clone(), direction: MoveDir::Up }).unwrap();

            let visible_order = |ed: &BoardEditor| -> Vec<ShortId> {
                ed.board()
                    .visible_cards(0, None)
                    .into_iter()
                    .map(|i| ed.board().lists[0].card_ids[i].clone())
                    .collect()
            };
            assert_eq!(visible_order(&editor), vec![bottom.clone(), top.clone()], "bottom moved above top");
            // Selection follows the moved card.
            assert_eq!(editor.board().current_card_id(), Some(&bottom));

            editor.reload().unwrap();
            assert_eq!(visible_order(&editor), vec![bottom.clone(), top.clone()], "survives reload");
        });
    }

    #[test]
    fn move_card_all_directions_persist_across_reload() {
        with_temp_dir(|| {
            let mut meta = BoardMeta::new("Test".into());
            let la = ids::new_id();
            let lb = ids::new_id();
            meta.lists.push(ListMeta { id: la.clone(), name: "A".into(), archived: false });
            meta.lists.push(ListMeta { id: lb.clone(), name: "B".into(), archived: false });
            board_store::save_board(&meta).unwrap();
            let mut a = Vec::new();
            for (i, t) in ["a0", "a1", "a2"].iter().enumerate() {
                let mut c = Card::new((*t).into());
                c.list_id = la.clone();
                c.position = (i + 1) as f64;
                card_store::save_card(&meta.id, &c).unwrap();
                a.push(c.id);
            }
            let mut editor = BoardEditor::load(&meta.id).unwrap();
            assert_eq!(editor.board().lists[0].card_ids, a);

            // Down, then reload.
            editor.apply(Command::MoveCard { card_id: a[0].clone(), direction: MoveDir::Down }).unwrap();
            let mem = editor.board().lists[0].card_ids.clone();
            assert_eq!(mem, vec![a[1].clone(), a[0].clone(), a[2].clone()]);
            editor.reload().unwrap();
            assert_eq!(editor.board().lists[0].card_ids, mem, "down survives reload");

            // Up brings it back.
            editor.apply(Command::MoveCard { card_id: a[0].clone(), direction: MoveDir::Up }).unwrap();
            editor.reload().unwrap();
            assert_eq!(editor.board().lists[0].card_ids, a, "up survives reload");

            // Right: a0 -> list B.
            editor.apply(Command::MoveCard { card_id: a[0].clone(), direction: MoveDir::Right }).unwrap();
            editor.reload().unwrap();
            assert_eq!(editor.board().lists[0].card_ids, vec![a[1].clone(), a[2].clone()], "src after right");
            assert_eq!(editor.board().lists[1].card_ids, vec![a[0].clone()], "dst after right");
            assert_eq!(editor.board().cards[&a[0]].list_id, lb);

            // Left: a0 back to list A.
            editor.apply(Command::MoveCard { card_id: a[0].clone(), direction: MoveDir::Left }).unwrap();
            editor.reload().unwrap();
            assert!(editor.board().lists[1].card_ids.is_empty(), "dst empty after left");
            assert!(editor.board().lists[0].card_ids.contains(&a[0]));
        });
    }

    // ── Persistence roundtrips: apply(Command) → reload() → assert on disk ──
    //
    // These guard the full mutate→persist→re-derive path (the gap that let the
    // archived-interleave move bug ship). Each loads a fresh editor, applies a
    // command, reloads from disk, and asserts the effect survived.

    use crate::model::label::LabelColor;

    /// Seed a board with one list and the given card titles (ascending
    /// positions), returning the editor plus the card ids in order.
    fn editor_with_cards(titles: &[&str]) -> (BoardEditor, ShortId, Vec<ShortId>) {
        let mut meta = BoardMeta::new("Test".into());
        let lid = ids::new_id();
        meta.lists.push(ListMeta { id: lid.clone(), name: "L".into(), archived: false });
        board_store::save_board(&meta).unwrap();
        let ids_v = titles
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let mut c = Card::new((*t).into());
                c.list_id = lid.clone();
                c.position = (i + 1) as f64;
                card_store::save_card(&meta.id, &c).unwrap();
                c.id
            })
            .collect();
        (BoardEditor::load(&meta.id).unwrap(), lid, ids_v)
    }

    #[test]
    fn add_card_persists_across_reload() {
        with_temp_dir(|| {
            let (mut ed, lid, _) = editor_with_cards(&["existing"]);
            ed.apply(Command::AddCard { list_id: lid.clone(), title: "fresh".into() }).unwrap();
            let new_id = ed.last_added_card_id().cloned().unwrap();
            ed.reload().unwrap();
            assert_eq!(ed.board().cards[&new_id].title, "fresh");
            assert_eq!(ed.board().cards[&new_id].list_id, lid);
            // Ordered after the existing card.
            assert_eq!(ed.board().lists[0].card_ids.last(), Some(&new_id));
        });
    }

    #[test]
    fn edit_title_and_description_persist_across_reload() {
        with_temp_dir(|| {
            let (mut ed, _, c) = editor_with_cards(&["orig"]);
            let id = c[0].clone();
            ed.apply(Command::EditCardTitle { card_id: id.clone(), title: "renamed".into() }).unwrap();
            ed.apply(Command::EditCardDescription { card_id: id.clone(), body: "body text".into() }).unwrap();
            ed.reload().unwrap();
            assert_eq!(ed.board().cards[&id].title, "renamed");
            assert_eq!(ed.board().cards[&id].description, "body text");
        });
    }

    #[test]
    fn due_date_set_then_clear_persists_across_reload() {
        with_temp_dir(|| {
            let (mut ed, _, c) = editor_with_cards(&["task"]);
            let id = c[0].clone();
            let date = chrono::NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
            ed.apply(Command::SetDueDate { card_id: id.clone(), date }).unwrap();
            ed.reload().unwrap();
            assert_eq!(ed.board().cards[&id].due_date, Some(date));
            ed.apply(Command::ClearDueDate { card_id: id.clone() }).unwrap();
            ed.reload().unwrap();
            assert_eq!(ed.board().cards[&id].due_date, None);
        });
    }

    #[test]
    fn archive_then_restore_card_persists_across_reload() {
        with_temp_dir(|| {
            let (mut ed, lid, c) = editor_with_cards(&["task"]);
            let id = c[0].clone();
            ed.apply(Command::ArchiveCard { card_id: id.clone() }).unwrap();
            ed.reload().unwrap();
            assert!(ed.board().cards[&id].archived);
            ed.apply(Command::RestoreCard { card_id: id.clone() }).unwrap();
            ed.reload().unwrap();
            assert!(!ed.board().cards[&id].archived);
            // Still a member of its original list, hence visible again.
            assert_eq!(ed.board().cards[&id].list_id, lid);
            assert!(ed.board().lists[0].card_ids.contains(&id));
        });
    }

    #[test]
    fn checklist_lifecycle_persists_across_reload() {
        with_temp_dir(|| {
            let (mut ed, _, c) = editor_with_cards(&["task"]);
            let id = c[0].clone();
            ed.apply(Command::AddChecklistItem { card_id: id.clone(), text: "one".into() }).unwrap();
            ed.apply(Command::AddChecklistItem { card_id: id.clone(), text: "two".into() }).unwrap();
            ed.apply(Command::AddChecklistItem { card_id: id.clone(), text: "three".into() }).unwrap();
            ed.apply(Command::ToggleChecklistItem { card_id: id.clone(), item_idx: 0 }).unwrap();
            ed.apply(Command::EditChecklistItem { card_id: id.clone(), item_idx: 1, text: "TWO".into() }).unwrap();
            ed.apply(Command::ReorderChecklistItem { card_id: id.clone(), from: 2, to: 0 }).unwrap();
            ed.reload().unwrap();
            let cl = &ed.board().cards[&id].checklist;
            let names: Vec<&str> = cl.iter().map(|i| i.text.as_str()).collect();
            assert_eq!(names, vec!["three", "one", "TWO"], "order after reorder");
            // "one" was completed; it is now at index 1 after the reorder.
            assert!(cl[1].completed);
            ed.apply(Command::RemoveChecklistItem { card_id: id.clone(), item_idx: 0 }).unwrap();
            ed.reload().unwrap();
            let names: Vec<&str> =
                ed.board().cards[&id].checklist.iter().map(|i| i.text.as_str()).collect();
            assert_eq!(names, vec!["one", "TWO"]);
        });
    }

    #[test]
    fn toggle_label_assignment_persists_across_reload() {
        with_temp_dir(|| {
            let (mut ed, _, c) = editor_with_cards(&["task"]);
            let id = c[0].clone();
            ed.apply(Command::DefineLabel { name: "urgent".into(), color: LabelColor::Red }).unwrap();
            let lbl = ed.board().meta.labels[0].id.clone();
            ed.apply(Command::ToggleLabel { card_id: id.clone(), label_id: lbl.clone() }).unwrap();
            ed.reload().unwrap();
            assert_eq!(ed.board().cards[&id].label_ids, vec![lbl.clone()]);
            ed.apply(Command::ToggleLabel { card_id: id.clone(), label_id: lbl.clone() }).unwrap();
            ed.reload().unwrap();
            assert!(ed.board().cards[&id].label_ids.is_empty());
        });
    }

    #[test]
    fn delete_label_prunes_from_card_on_next_save() {
        with_temp_dir(|| {
            let (mut ed, _, c) = editor_with_cards(&["task"]);
            let id = c[0].clone();
            ed.apply(Command::DefineLabel { name: "temp".into(), color: LabelColor::Green }).unwrap();
            let lbl = ed.board().meta.labels[0].id.clone();
            ed.apply(Command::ToggleLabel { card_id: id.clone(), label_id: lbl.clone() }).unwrap();
            ed.apply(Command::DeleteLabel { label_id: lbl.clone() }).unwrap();
            // Lazy prune: the dead id is shed on the card's next save.
            ed.apply(Command::EditCardTitle { card_id: id.clone(), title: "touched".into() }).unwrap();
            ed.reload().unwrap();
            assert!(ed.board().meta.labels.is_empty());
            assert!(ed.board().cards[&id].label_ids.is_empty(), "dead label id pruned on save");
        });
    }

    #[test]
    fn label_meta_commands_persist_across_reload() {
        with_temp_dir(|| {
            let (mut ed, _, _) = editor_with_cards(&["task"]);
            ed.apply(Command::DefineLabel { name: "a".into(), color: LabelColor::Red }).unwrap();
            ed.apply(Command::DefineLabel { name: "b".into(), color: LabelColor::Green }).unwrap();
            let a = ed.board().meta.labels[0].id.clone();
            ed.apply(Command::RenameLabel { label_id: a.clone(), name: "alpha".into() }).unwrap();
            ed.apply(Command::SetLabelColor { label_id: a.clone(), color: LabelColor::Blue }).unwrap();
            ed.apply(Command::ReorderLabels { from: 0, to: 1 }).unwrap();
            ed.reload().unwrap();
            // After reorder, "alpha" (the renamed/recolored one) is last.
            let labels = &ed.board().meta.labels;
            assert_eq!(labels.len(), 2);
            assert_eq!(labels[1].name, "alpha");
            assert_eq!(labels[1].color, LabelColor::Blue);
            assert_eq!(labels[0].name, "b");
        });
    }

    #[test]
    fn list_commands_persist_across_reload() {
        with_temp_dir(|| {
            let (mut ed, lid, _) = editor_with_cards(&["task"]);
            ed.apply(Command::AddList { name: "Second".into() }).unwrap();
            ed.apply(Command::RenameList { list_id: lid.clone(), name: "First".into() }).unwrap();
            ed.reload().unwrap();
            assert_eq!(ed.board().lists.len(), 2);
            assert_eq!(ed.board().lists[0].name, "First");
            assert_eq!(ed.board().lists[1].name, "Second");

            // Reorder: move "First" right (down the meta order).
            ed.apply(Command::MoveList { list_id: lid.clone(), direction: MoveDir::Right }).unwrap();
            ed.reload().unwrap();
            assert_eq!(ed.board().lists[0].name, "Second");
            assert_eq!(ed.board().lists[1].name, "First");
        });
    }

    #[test]
    fn archive_then_restore_list_persists_across_reload() {
        with_temp_dir(|| {
            let (mut ed, lid, _) = editor_with_cards(&["task"]);
            ed.apply(Command::AddList { name: "Keep".into() }).unwrap();
            ed.apply(Command::ArchiveList { list_id: lid.clone() }).unwrap();
            ed.reload().unwrap();
            // Active view excludes the archived list.
            assert_eq!(ed.board().lists.len(), 1);
            assert_eq!(ed.board().lists[0].name, "Keep");

            ed.apply(Command::RestoreList { list_id: lid.clone() }).unwrap();
            ed.reload().unwrap();
            assert_eq!(ed.board().lists.len(), 2);
            assert!(ed.board().lists.iter().any(|l| l.id == lid));
        });
    }

    #[test]
    fn board_commands_persist_across_reload() {
        with_temp_dir(|| {
            let (mut ed, _, _) = editor_with_cards(&["task"]);
            let bid = ed.board().meta.id.clone();
            ed.apply(Command::RenameBoard { name: "Renamed Board".into() }).unwrap();
            ed.apply(Command::SetAccentColor { color: LabelColor::Blue }).unwrap();
            ed.reload().unwrap();
            assert_eq!(ed.board().meta.name, "Renamed Board");
            assert_eq!(ed.board().meta.accent_color, LabelColor::Blue);

            ed.apply(Command::ArchiveBoard { board_id: bid.clone() }).unwrap();
            ed.reload().unwrap();
            assert!(ed.board().meta.archived);
            ed.apply(Command::RestoreBoard { board_id: bid.clone() }).unwrap();
            ed.reload().unwrap();
            assert!(!ed.board().meta.archived);
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
            detail_max_scroll: std::cell::Cell::new(0),
        }
    }

    // Build a single-list board, one card homed in that list. `mutate` can
    // break it before the invariant check runs.
    fn invariant_board(mutate: impl FnOnce(&mut LoadedBoard)) -> BoardEditor {
        let list_id = "lst00001".to_string();
        let mut meta = BoardMeta::new("X".into());
        meta.lists.push(ListMeta { id: list_id.clone(), name: "L".into(), archived: false });
        let mut card = fixed_card("c0000001", "a");
        card.list_id = list_id;
        card.position = 1.0;
        let cards: HashMap<_, _> = std::iter::once((card.id.clone(), card)).collect();
        let lists = crate::model::list::build_lists(&meta, &cards, false);
        let mut board = LoadedBoard {
            meta,
            lists,
            cards,
            selected_list: 0,
            selected_card: vec![0],
            scroll_offset: vec![0],
            detail_item_idx: 0,
            detail_scroll: 0,
            detail_max_scroll: std::cell::Cell::new(0),
        };
        mutate(&mut board);
        BoardEditor::from_loaded(board)
    }

    #[test]
    fn check_invariants_passes_on_well_formed_board() {
        let editor = invariant_board(|_| {});
        assert!(editor.check_invariants().is_ok());
    }

    #[test]
    fn check_invariants_detects_orphan_card() {
        let editor = invariant_board(|b| {
            let id = b.lists[0].card_ids[0].clone();
            b.cards.get_mut(&id).unwrap().list_id = "ghostlst".into();
        });
        let err = editor.check_invariants().unwrap_err();
        assert!(err.contains("naming no known List"), "got: {err}");
    }

    #[test]
    fn check_invariants_detects_selection_out_of_range() {
        let editor = invariant_board(|b| b.selected_card[0] = 5);
        let err = editor.check_invariants().unwrap_err();
        assert!(err.contains("ordinal"), "got: {err}");
    }

    #[test]
    fn check_invariants_detects_diverged_card_ids() {
        let editor = invariant_board(|b| b.lists[0].card_ids.push("phantom0".into()));
        let err = editor.check_invariants().unwrap_err();
        assert!(err.contains("diverged"), "got: {err}");
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
