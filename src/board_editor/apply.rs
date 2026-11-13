use super::*;
use crate::command::Command;
use crate::model::board::ListMeta;
use crate::model::card::{Card, ChecklistItem, HistoryField};
use crate::model::id::ListId;
use crate::model::label::Label;
use crate::model::list::CardList;

impl BoardEditor {
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
                pending.push(PendingWrite::Card(Box::new(card.clone())));
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
                pending.push(PendingWrite::Card(Box::new(card.clone())));
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
                let old = std::mem::replace(&mut card.title, title);
                if changed {
                    let new = card.title.clone();
                    card.log_change(HistoryField::Title, "Edited title", Some(old), Some(new));
                } else {
                    card.touch();
                }
                pending.push(PendingWrite::Card(Box::new(card.clone())));
            }
            Command::EditCardDescription {
                card_id,
                body,
                cursor,
            } => {
                let card = self.card_mut(&card_id)?;
                if cursor.is_some() {
                    card.description_cursor = cursor;
                }
                let changed = card.description != body;
                let old = std::mem::replace(&mut card.description, body);
                if changed {
                    let new = card.description.clone();
                    card.log_change(
                        HistoryField::Description,
                        "Edited description",
                        Some(old),
                        Some(new),
                    );
                } else {
                    card.touch();
                }
                pending.push(PendingWrite::Card(Box::new(card.clone())));
            }
            Command::SetDueDate { card_id, date } => {
                let card = self.card_mut(&card_id)?;
                let prev = card.due_date;
                card.due_date = Some(date);
                if prev != Some(date) {
                    card.log_change(
                        HistoryField::DueDate,
                        format!("Set due date to {date}"),
                        prev.map(|d| d.to_string()),
                        Some(date.to_string()),
                    );
                } else {
                    card.touch();
                }
                pending.push(PendingWrite::Card(Box::new(card.clone())));
            }
            Command::ClearDueDate { card_id } => {
                let card = self.card_mut(&card_id)?;
                let prev = card.due_date.take();
                if let Some(prev) = prev {
                    card.log_change(
                        HistoryField::DueDate,
                        "Cleared due date",
                        Some(prev.to_string()),
                        None,
                    );
                } else {
                    card.touch();
                }
                pending.push(PendingWrite::Card(Box::new(card.clone())));
            }
            Command::AddChecklistItem { card_id, text } => {
                let card = self.card_mut(&card_id)?;
                let action = format!("Added checklist item '{text}'");
                let old = card.checklist_as_markdown();
                card.checklist.push(ChecklistItem {
                    text,
                    completed: false,
                });
                card.log_checklist(action, old);
                let last = card.checklist.len() - 1;
                pending.push(PendingWrite::Card(Box::new(card.clone())));
                // Detail cursor follows the new item.
                self.board.detail_item_idx = last;
            }
            Command::EditChecklistItem {
                card_id,
                item_idx,
                text,
            } => {
                let card = self.card_mut(&card_id)?;
                let old_checklist = card.checklist_as_markdown();
                let item = card
                    .checklist
                    .get_mut(item_idx)
                    .ok_or(BoardEditorError::Invariant("checklist index out of range"))?;
                let changed = item.text != text;
                let old_text = std::mem::replace(&mut item.text, text);
                let new_text = item.text.clone();
                if changed {
                    let was = old_checklist;
                    card.log_checklist(
                        format!("Renamed checklist item '{old_text}' → '{new_text}'"),
                        was,
                    );
                } else {
                    card.touch();
                }
                pending.push(PendingWrite::Card(Box::new(card.clone())));
            }
            Command::ToggleChecklistItem { card_id, item_idx } => {
                let card = self.card_mut(&card_id)?;
                let old = card.checklist_as_markdown();
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
                card.log_checklist(action, old);
                pending.push(PendingWrite::Card(Box::new(card.clone())));
            }
            Command::RemoveChecklistItem { card_id, item_idx } => {
                let card = self.card_mut(&card_id)?;
                if item_idx >= card.checklist.len() {
                    return Err(BoardEditorError::Invariant("checklist index out of range"));
                }
                let old = card.checklist_as_markdown();
                let removed = card.checklist.remove(item_idx);
                card.log_checklist(format!("Removed checklist item '{}'", removed.text), old);
                let len = card.checklist.len();
                pending.push(PendingWrite::Card(Box::new(card.clone())));
                // Clamp detail cursor to the shrunk checklist.
                if self.board.detail_item_idx >= len && len > 0 {
                    self.board.detail_item_idx = len - 1;
                }
            }
            Command::ReorderChecklistItem { card_id, from, to } => {
                let card = self.card_mut(&card_id)?;
                let len = card.checklist.len();
                if from >= len || to >= len {
                    return Err(BoardEditorError::Invariant("checklist index out of range"));
                }
                if from != to {
                    let old = card.checklist_as_markdown();
                    let name = card.checklist[from].text.clone();
                    let item = card.checklist.remove(from);
                    card.checklist.insert(to, item);
                    card.log_checklist(format!("Reordered checklist item '{name}'"), old);
                } else {
                    card.touch();
                }
                pending.push(PendingWrite::Card(Box::new(card.clone())));
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
                        id: label_id.to_string(),
                    })?;
                let board_labels = &self.board.meta.labels;
                let card =
                    self.board
                        .cards
                        .get_mut(&card_id)
                        .ok_or(BoardEditorError::NotFound {
                            kind: "card",
                            id: card_id.to_string(),
                        })?;
                let old = card.label_names_text(board_labels);
                let action = if let Some(pos) = card.label_ids.iter().position(|id| *id == label_id)
                {
                    card.label_ids.remove(pos);
                    format!("Removed label '{label_name}'")
                } else {
                    card.label_ids.push(label_id);
                    format!("Added label '{label_name}'")
                };
                let new = card.label_names_text(board_labels);
                card.log_change(HistoryField::Labels, action, Some(old), Some(new));
                pending.push(PendingWrite::Card(Box::new(card.clone())));
            }
            Command::DeleteHistoryEntry { card_id, index } => {
                let card = self.card_mut(&card_id)?;
                if index >= card.history.len() {
                    return Err(BoardEditorError::Invariant("history index out of range"));
                }
                card.history.remove(index);
                card.touch();
                pending.push(PendingWrite::Card(Box::new(card.clone())));
            }
            Command::AddCard { list_id, title } => {
                let li = self
                    .board
                    .lists
                    .iter()
                    .position(|l| l.id == list_id)
                    .ok_or_else(|| BoardEditorError::NotFound {
                        kind: "list",
                        id: list_id.to_string(),
                    })?;
                let pos = match self.board.meta.new_card_position {
                    crate::model::board::NewCardPosition::Bottom => self.next_position(&list_id),
                    crate::model::board::NewCardPosition::Top => self.first_position(&list_id),
                };
                let mut card = Card::new(title);
                card.list_id = list_id.clone();
                card.position = pos;
                card.log("Created");
                let new_id = card.id.clone();
                self.board.cards.insert(card.id.clone(), card.clone());
                self.rebuild_list(li);
                pending.push(PendingWrite::Card(Box::new(card)));
                self.last_added_card_id = Some(new_id);
                // Selection follows the new card — it is non-archived and last/first
                // in the list, so its visible ordinal is the last/first visible slot.
                self.board.selected_list = li;
                let ord = match self.board.meta.new_card_position {
                    crate::model::board::NewCardPosition::Bottom => {
                        self.board.visible_cards(li, None).len().saturating_sub(1)
                    }
                    crate::model::board::NewCardPosition::Top => 0,
                };
                if let Some(slot) = self.board.selected_card.get_mut(li) {
                    *slot = ord;
                }
                if self.board.meta.new_card_position == crate::model::board::NewCardPosition::Top
                    && let Some(offset) = self.board.scroll_offset.get(li)
                {
                    offset.set(0);
                }
            }
            Command::MoveCard { card_id, direction } => {
                self.move_card(&card_id, direction, &mut pending)?;
            }
            Command::AddList { name } => {
                let meta = ListMeta {
                    id: ListId::generate(),
                    name,
                    archived: false,
                };
                self.board.lists.push(CardList {
                    id: meta.id.clone(),
                    name: meta.name.clone(),
                    card_ids: Vec::new(),
                });
                self.board.meta.lists.push(meta);
                self.board.selected_card.push(0);
                self.board.scroll_offset.push(std::cell::Cell::new(0));
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
                        id: list_id.to_string(),
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
                        id: list_id.to_string(),
                    })?;
                lm.archived = false;
                let name = lm.name.clone();
                // Cards were never moved; rebuild the active-list row from them.
                let card_ids = crate::model::list::ordered_card_ids(&list_id, &self.board.cards);
                self.board.lists.push(CardList {
                    id: list_id.clone(),
                    name,
                    card_ids,
                });
                self.board.selected_card.push(0);
                self.board.scroll_offset.push(std::cell::Cell::new(0));
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
                        id: list_id.to_string(),
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
                        id: board_id.to_string(),
                    });
                }
                self.board.meta.archived = true;
                pending.push(PendingWrite::Board);
            }
            Command::RestoreBoard { board_id } => {
                if board_id != self.board.meta.id {
                    return Err(BoardEditorError::NotFound {
                        kind: "board",
                        id: board_id.to_string(),
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
            Command::SetNewCardPosition { position } => {
                self.board.meta.new_card_position = position;
                pending.push(PendingWrite::Board);
            }
            Command::SetShowCreatedDate { show } => {
                self.board.meta.show_created_date = show;
                pending.push(PendingWrite::Board);
            }
            Command::SetShowUpdatedDate { show } => {
                self.board.meta.show_updated_date = show;
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
                        id: label_id.to_string(),
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
                        id: label_id.to_string(),
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
                        id: label_id.to_string(),
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
}
