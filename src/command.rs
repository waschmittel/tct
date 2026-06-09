//! Domain mutation intent as a value. Applied by `BoardEditor::apply`.
//!
//! See `docs/adr/0002-command-enum-scope.md`. Selection moves are NOT
//! commands — they are direct methods on `BoardEditor`.

use crate::model::ids::ShortId;
use crate::model::label::LabelColor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveDir {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub enum Command {
    // Card commands
    ArchiveCard { card_id: ShortId },
    RestoreCard { card_id: ShortId },
    EditCardTitle { card_id: ShortId, title: String },
    EditCardDescription { card_id: ShortId, body: String },
    SetDueDate { card_id: ShortId, date: chrono::NaiveDate },
    ClearDueDate { card_id: ShortId },
    AddChecklistItem { card_id: ShortId, text: String },
    EditChecklistItem { card_id: ShortId, item_idx: usize, text: String },
    ToggleChecklistItem { card_id: ShortId, item_idx: usize },
    RemoveChecklistItem { card_id: ShortId, item_idx: usize },
    ReorderChecklistItem { card_id: ShortId, from: usize, to: usize },
    ToggleLabel { card_id: ShortId, label_id: ShortId },
    AddCard { list_id: ShortId, title: String },
    MoveCard { card_id: ShortId, direction: MoveDir },

    // List commands
    AddList { name: String },
    ArchiveList { list_id: ShortId },
    RestoreList { list_id: ShortId },
    RenameList { list_id: ShortId, name: String },
    MoveList { list_id: ShortId, direction: MoveDir },

    // Board commands (operate on the loaded board)
    ArchiveBoard { board_id: ShortId },
    RestoreBoard { board_id: ShortId },
    RenameBoard { name: String },
    SetAccentColor { color: LabelColor },

    // Label commands
    DefineLabel { name: String, color: LabelColor },
    RenameLabel { label_id: ShortId, name: String },
    SetLabelColor { label_id: ShortId, color: LabelColor },
    DeleteLabel { label_id: ShortId },
}
