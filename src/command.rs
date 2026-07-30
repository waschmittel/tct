//! Domain mutation intent as a value. Applied by `BoardEditor::apply`.
//!
//! See `docs/adr/0002-command-enum-scope.md`. Selection moves are NOT
//! commands — they are direct methods on `BoardEditor`.

use crate::model::id::{BoardId, CardId, LabelId, ListId};
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
    ArchiveCard { card_id: CardId },
    RestoreCard { card_id: CardId },
    EditCardTitle { card_id: CardId, title: String },
    EditCardDescription { card_id: CardId, body: String },
    SetDueDate { card_id: CardId, date: chrono::NaiveDate },
    ClearDueDate { card_id: CardId },
    AddChecklistItem { card_id: CardId, text: String },
    EditChecklistItem { card_id: CardId, item_idx: usize, text: String },
    ToggleChecklistItem { card_id: CardId, item_idx: usize },
    RemoveChecklistItem { card_id: CardId, item_idx: usize },
    ReorderChecklistItem { card_id: CardId, from: usize, to: usize },
    ToggleLabel { card_id: CardId, label_id: LabelId },
    AddCard { list_id: ListId, title: String },
    MoveCard { card_id: CardId, direction: MoveDir },

    // List commands
    AddList { name: String },
    ArchiveList { list_id: ListId },
    RestoreList { list_id: ListId },
    RenameList { list_id: ListId, name: String },
    MoveList { list_id: ListId, direction: MoveDir },

    // Board commands (operate on the loaded board)
    ArchiveBoard { board_id: BoardId },
    RestoreBoard { board_id: BoardId },
    RenameBoard { name: String },
    SetAccentColor { color: LabelColor },

    // Label commands
    DefineLabel { name: String, color: LabelColor },
    RenameLabel { label_id: LabelId, name: String },
    SetLabelColor { label_id: LabelId, color: LabelColor },
    DeleteLabel { label_id: LabelId },
    ReorderLabels { from: usize, to: usize },
}
