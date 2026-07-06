//! Single-line text-input insert handlers (titles, names, items).
//!
//! All share the [`LineInput`](super::line_input::LineInput) primitive
//! and differ only in:
//! - what surface they render on (`BoardView`, `BoardSelector`,
//!   `CardDetail`),
//! - the popup title,
//! - what command (or side effect) they emit on confirm.

use crossterm::event::KeyEvent;

use super::line_input::{LineInput, LineKey};
use super::{InsertHandler, InsertOutcome, InsertSideEffect, InsertSurface};
use crate::app::LoadedBoard;
use crate::command::Command;
use crate::dialog::label_manager::LabelManager;
use crate::model::ids::ShortId;

// ── Helpers ──────────────────────────────────────────────────────────

fn rejected_empty_outcome() -> InsertOutcome {
    InsertOutcome::Cancel
}

// ── NewBoardName ─────────────────────────────────────────────────────

pub struct NewBoardName {
    pub input: LineInput,
}
impl NewBoardName {
    pub fn new() -> Self { Self { input: LineInput::new() } }
}
impl InsertHandler for NewBoardName {
    fn handle_key(&mut self, key: KeyEvent, _b: Option<&LoadedBoard>) -> InsertOutcome {
        match self.input.handle_key(key) {
            LineKey::Confirm => {
                let text = self.input.trimmed();
                if text.is_empty() {
                    return rejected_empty_outcome();
                }
                InsertOutcome::ConfirmSideEffect(Box::new(InsertSideEffect::CreateBoard {
                    name: text,
                }))
            }
            LineKey::Cancel => InsertOutcome::Cancel,
            _ => InsertOutcome::Stay,
        }
    }
    fn surface(&self) -> InsertSurface { InsertSurface::BoardSelector }
    fn title(&self) -> &str { "New Board" }
    fn line_buffer(&self) -> Option<&str> { Some(&self.input.buffer) }
    fn line_cursor(&self) -> Option<usize> { Some(self.input.cursor) }
    fn as_any(&self) -> &dyn std::any::Any { self }
}

// ── RenameBoard ──────────────────────────────────────────────────────

pub struct RenameBoard {
    pub input: LineInput,
}
impl RenameBoard {
    pub fn new(current: &str) -> Self { Self { input: LineInput::with_initial(current) } }
}
impl InsertHandler for RenameBoard {
    fn handle_key(&mut self, key: KeyEvent, _b: Option<&LoadedBoard>) -> InsertOutcome {
        match self.input.handle_key(key) {
            LineKey::Confirm => {
                let text = self.input.trimmed();
                if text.is_empty() {
                    return rejected_empty_outcome();
                }
                InsertOutcome::ConfirmSideEffect(Box::new(
                    InsertSideEffect::RenameSelectedBoard { new_name: text },
                ))
            }
            LineKey::Cancel => InsertOutcome::Cancel,
            _ => InsertOutcome::Stay,
        }
    }
    fn surface(&self) -> InsertSurface { InsertSurface::BoardSelector }
    fn title(&self) -> &str { "Rename Board" }
    fn line_buffer(&self) -> Option<&str> { Some(&self.input.buffer) }
    fn line_cursor(&self) -> Option<usize> { Some(self.input.cursor) }
    fn as_any(&self) -> &dyn std::any::Any { self }
}

// ── NewCardTitle ─────────────────────────────────────────────────────

pub struct NewCardTitle {
    pub input: LineInput,
}
impl NewCardTitle {
    pub fn new() -> Self { Self { input: LineInput::new() } }
}
impl InsertHandler for NewCardTitle {
    fn handle_key(&mut self, key: KeyEvent, board: Option<&LoadedBoard>) -> InsertOutcome {
        match self.input.handle_key(key) {
            LineKey::Confirm => {
                let text = self.input.trimmed();
                if text.is_empty() {
                    return rejected_empty_outcome();
                }
                let list_id = board
                    .and_then(|b| b.lists.get(b.selected_list).map(|l| l.id.clone()));
                match list_id {
                    Some(list_id) => {
                        InsertOutcome::Confirm(Command::AddCard { list_id, title: text })
                    }
                    None => InsertOutcome::Cancel,
                }
            }
            LineKey::Cancel => InsertOutcome::Cancel,
            _ => InsertOutcome::Stay,
        }
    }
    fn surface(&self) -> InsertSurface { InsertSurface::BoardView }
    fn title(&self) -> &str { "New Card" }
    fn line_buffer(&self) -> Option<&str> { Some(&self.input.buffer) }
    fn line_cursor(&self) -> Option<usize> { Some(self.input.cursor) }
    fn as_any(&self) -> &dyn std::any::Any { self }
}

// ── EditCardTitle (over card detail) and EditCardTitleInline (over board view)

pub struct EditCardTitle {
    pub input: LineInput,
    pub inline: bool,
    pub card_id: ShortId,
}
impl EditCardTitle {
    pub fn new(card_id: ShortId, current: &str, inline: bool) -> Self {
        Self { input: LineInput::with_initial(current), inline, card_id }
    }
}
impl InsertHandler for EditCardTitle {
    fn handle_key(&mut self, key: KeyEvent, _b: Option<&LoadedBoard>) -> InsertOutcome {
        match self.input.handle_key(key) {
            LineKey::Confirm => {
                let text = self.input.trimmed();
                if text.is_empty() {
                    return InsertOutcome::Cancel;
                }
                InsertOutcome::Confirm(Command::EditCardTitle {
                    card_id: self.card_id.clone(),
                    title: text,
                })
            }
            LineKey::Cancel => InsertOutcome::Cancel,
            _ => InsertOutcome::Stay,
        }
    }
    fn surface(&self) -> InsertSurface {
        if self.inline { InsertSurface::BoardView } else { InsertSurface::CardDetail }
    }
    fn title(&self) -> &str { "Edit Card Title" }
    fn line_buffer(&self) -> Option<&str> { Some(&self.input.buffer) }
    fn line_cursor(&self) -> Option<usize> { Some(self.input.cursor) }
    fn as_any(&self) -> &dyn std::any::Any { self }
}

// ── NewListName ──────────────────────────────────────────────────────

pub struct NewListName {
    pub input: LineInput,
}
impl NewListName {
    pub fn new() -> Self { Self { input: LineInput::new() } }
}
impl InsertHandler for NewListName {
    fn handle_key(&mut self, key: KeyEvent, _b: Option<&LoadedBoard>) -> InsertOutcome {
        match self.input.handle_key(key) {
            LineKey::Confirm => {
                let text = self.input.trimmed();
                if text.is_empty() {
                    return rejected_empty_outcome();
                }
                InsertOutcome::Confirm(Command::AddList { name: text })
            }
            LineKey::Cancel => InsertOutcome::Cancel,
            _ => InsertOutcome::Stay,
        }
    }
    fn surface(&self) -> InsertSurface { InsertSurface::BoardView }
    fn title(&self) -> &str { "New List" }
    fn line_buffer(&self) -> Option<&str> { Some(&self.input.buffer) }
    fn line_cursor(&self) -> Option<usize> { Some(self.input.cursor) }
    fn as_any(&self) -> &dyn std::any::Any { self }
}

// ── RenameList ───────────────────────────────────────────────────────

pub struct RenameList {
    pub input: LineInput,
    pub list_id: ShortId,
}
impl RenameList {
    pub fn new(list_id: ShortId, current: &str) -> Self {
        Self { input: LineInput::with_initial(current), list_id }
    }
}
impl InsertHandler for RenameList {
    fn handle_key(&mut self, key: KeyEvent, _b: Option<&LoadedBoard>) -> InsertOutcome {
        match self.input.handle_key(key) {
            LineKey::Confirm => {
                let text = self.input.trimmed();
                if text.is_empty() {
                    return InsertOutcome::Cancel;
                }
                InsertOutcome::Confirm(Command::RenameList {
                    list_id: self.list_id.clone(),
                    name: text,
                })
            }
            LineKey::Cancel => InsertOutcome::Cancel,
            _ => InsertOutcome::Stay,
        }
    }
    fn surface(&self) -> InsertSurface { InsertSurface::BoardView }
    fn title(&self) -> &str { "Rename List" }
    fn line_buffer(&self) -> Option<&str> { Some(&self.input.buffer) }
    fn line_cursor(&self) -> Option<usize> { Some(self.input.cursor) }
    fn as_any(&self) -> &dyn std::any::Any { self }
}

// ── NewChecklistItem ─────────────────────────────────────────────────

pub struct NewChecklistItem {
    pub input: LineInput,
    pub card_id: ShortId,
}
impl NewChecklistItem {
    pub fn new(card_id: ShortId) -> Self { Self { input: LineInput::new(), card_id } }
}
impl InsertHandler for NewChecklistItem {
    fn handle_key(&mut self, key: KeyEvent, _b: Option<&LoadedBoard>) -> InsertOutcome {
        match self.input.handle_key(key) {
            LineKey::Confirm => {
                let text = self.input.trimmed();
                if text.is_empty() {
                    return InsertOutcome::Cancel;
                }
                InsertOutcome::Confirm(Command::AddChecklistItem {
                    card_id: self.card_id.clone(),
                    text,
                })
            }
            LineKey::Cancel => InsertOutcome::Cancel,
            _ => InsertOutcome::Stay,
        }
    }
    fn surface(&self) -> InsertSurface { InsertSurface::CardDetail }
    fn title(&self) -> &str { "New Item" }
    fn line_buffer(&self) -> Option<&str> { Some(&self.input.buffer) }
    fn line_cursor(&self) -> Option<usize> { Some(self.input.cursor) }
    fn as_any(&self) -> &dyn std::any::Any { self }
}

// ── EditChecklistItem ────────────────────────────────────────────────

pub struct EditChecklistItem {
    pub input: LineInput,
    pub card_id: ShortId,
    pub item_idx: usize,
}
impl EditChecklistItem {
    pub fn new(card_id: ShortId, item_idx: usize, current: &str) -> Self {
        Self { input: LineInput::with_initial(current), card_id, item_idx }
    }
}
impl InsertHandler for EditChecklistItem {
    fn handle_key(&mut self, key: KeyEvent, _b: Option<&LoadedBoard>) -> InsertOutcome {
        match self.input.handle_key(key) {
            LineKey::Confirm => {
                let text = self.input.trimmed();
                if text.is_empty() {
                    return InsertOutcome::Cancel;
                }
                InsertOutcome::Confirm(Command::EditChecklistItem {
                    card_id: self.card_id.clone(),
                    item_idx: self.item_idx,
                    text,
                })
            }
            LineKey::Cancel => InsertOutcome::Cancel,
            _ => InsertOutcome::Stay,
        }
    }
    fn surface(&self) -> InsertSurface { InsertSurface::CardDetail }
    fn title(&self) -> &str { "Edit Item" }
    fn line_buffer(&self) -> Option<&str> { Some(&self.input.buffer) }
    fn line_cursor(&self) -> Option<usize> { Some(self.input.cursor) }
    fn as_any(&self) -> &dyn std::any::Any { self }
}

// ── NewLabelName ─────────────────────────────────────────────────────

pub struct NewLabelName {
    pub input: LineInput,
    /// Label index to keep selected in the LabelManager follow-up
    /// (set by the dispatcher after confirm — appended).
    pub label_selected_idx: usize,
    /// Threaded through so the reopened LabelManager still returns to
    /// the LabelPicker on close.
    pub from_picker: bool,
}
impl NewLabelName {
    pub fn new(from_picker: bool) -> Self {
        Self { input: LineInput::new(), label_selected_idx: 0, from_picker }
    }
}
impl InsertHandler for NewLabelName {
    fn handle_key(&mut self, key: KeyEvent, board: Option<&LoadedBoard>) -> InsertOutcome {
        match self.input.handle_key(key) {
            LineKey::Confirm => {
                let text = self.input.trimmed();
                if text.is_empty() {
                    return InsertOutcome::OpenDialog(Box::new(LabelManager {
                        selected_idx: self.label_selected_idx,
                        from_picker: self.from_picker,
                    }));
                }
                let color = board
                    .map(|b| {
                        let existing: Vec<_> = b.meta.labels.iter().map(|l| l.color).collect();
                        crate::model::label::LabelColor::generate_pastel(&existing)
                    })
                    .unwrap_or(crate::model::label::LabelColor::Red);
                let new_idx = board.map(|b| b.meta.labels.len()).unwrap_or(0);
                InsertOutcome::ConfirmAndOpenDialog(
                    Command::DefineLabel { name: text, color },
                    Box::new(LabelManager {
                        selected_idx: new_idx,
                        from_picker: self.from_picker,
                    }),
                )
            }
            LineKey::Cancel => InsertOutcome::OpenDialog(Box::new(LabelManager {
                selected_idx: self.label_selected_idx,
                from_picker: self.from_picker,
            })),
            _ => InsertOutcome::Stay,
        }
    }
    fn surface(&self) -> InsertSurface { InsertSurface::BoardView }
    fn title(&self) -> &str { "New Label" }
    fn line_buffer(&self) -> Option<&str> { Some(&self.input.buffer) }
    fn line_cursor(&self) -> Option<usize> { Some(self.input.cursor) }
    fn as_any(&self) -> &dyn std::any::Any { self }
}

// ── EditLabelName ────────────────────────────────────────────────────

pub struct EditLabelName {
    pub input: LineInput,
    pub label_id: ShortId,
    pub label_idx: usize,
    /// Threaded through so the reopened LabelManager still returns to
    /// the LabelPicker on close.
    pub from_picker: bool,
}
impl EditLabelName {
    pub fn new(label_id: ShortId, label_idx: usize, current: &str, from_picker: bool) -> Self {
        Self { input: LineInput::with_initial(current), label_id, label_idx, from_picker }
    }
}
impl InsertHandler for EditLabelName {
    fn handle_key(&mut self, key: KeyEvent, _b: Option<&LoadedBoard>) -> InsertOutcome {
        match self.input.handle_key(key) {
            LineKey::Confirm => {
                let text = self.input.trimmed();
                if text.is_empty() {
                    return InsertOutcome::OpenDialog(Box::new(LabelManager {
                        selected_idx: self.label_idx,
                        from_picker: self.from_picker,
                    }));
                }
                InsertOutcome::ConfirmAndOpenDialog(
                    Command::RenameLabel {
                        label_id: self.label_id.clone(),
                        name: text,
                    },
                    Box::new(LabelManager {
                        selected_idx: self.label_idx,
                        from_picker: self.from_picker,
                    }),
                )
            }
            LineKey::Cancel => InsertOutcome::OpenDialog(Box::new(LabelManager {
                selected_idx: self.label_idx,
                from_picker: self.from_picker,
            })),
            _ => InsertOutcome::Stay,
        }
    }
    fn surface(&self) -> InsertSurface { InsertSurface::BoardView }
    fn title(&self) -> &str { "Rename Label" }
    fn line_buffer(&self) -> Option<&str> { Some(&self.input.buffer) }
    fn line_cursor(&self) -> Option<usize> { Some(self.input.cursor) }
    fn as_any(&self) -> &dyn std::any::Any { self }
}
