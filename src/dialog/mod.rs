//! Modal dialog handlers. Each dialog kind is a struct implementing
//! [`Dialog`]; the active dialog lives on `App.dialog`.
//!
//! Adding a new dialog: create `src/dialog/<name>.rs` with a struct
//! holding the dialog's payload (raw IDs, not `Command`s — they are
//! built at confirm time so they refresh from the current Board Editor
//! state), implement [`Dialog`], and open it via `app.open_dialog(...)`.

use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::app::LoadedBoard;
use crate::command::Command;

pub mod archived_boards;
pub mod archived_cards;
pub mod archived_lists;
pub mod card_history;
pub(crate) mod common;
pub mod confirm_archive_board;
pub mod confirm_archive_card;
pub mod confirm_archive_list;
pub mod confirm_cancel_edit;
pub mod confirm_delete_label;
pub mod label_manager;
pub mod label_picker;

/// What rendering layer a dialog overlays.
///
/// `ArchiveBoard` / `ArchivedBoards` are board-level dialogs that need the
/// **Board Selector** as background. Everything else overlays the current
/// view (whatever `App.previous_mode` resolves to).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogBackground {
    /// Use the current view as background (default).
    Auto,
    /// Force the **Board Selector** as background.
    BoardSelector,
}

/// Outcome of dispatching a key event to a dialog.
///
/// Composed by the dispatcher: an outcome may carry an optional command
/// to apply, an optional status message, and a follow-up action.
pub struct DialogOutcome {
    pub apply: Option<Command>,
    pub side_effect: Option<DialogSideEffect>,
    pub status: Option<String>,
    pub follow: Follow,
}

/// What the dispatcher should do with the dialog after handling.
pub enum Follow {
    /// Dialog stays open.
    Stay,
    /// Close the dialog and restore `previous_mode`.
    Close,
    /// Close to a specific target mode (does not consume `previous_mode`).
    CloseTo(crate::app::AppMode),
    /// Replace this dialog with another.
    Open(Box<dyn Dialog>),
}

impl DialogOutcome {
    pub fn stay() -> Self {
        Self {
            apply: None,
            side_effect: None,
            status: None,
            follow: Follow::Stay,
        }
    }
    pub fn close() -> Self {
        Self {
            apply: None,
            side_effect: None,
            status: None,
            follow: Follow::Close,
        }
    }
    pub fn close_to(mode: crate::app::AppMode) -> Self {
        Self {
            apply: None,
            side_effect: None,
            status: None,
            follow: Follow::CloseTo(mode),
        }
    }
    pub fn apply(cmd: Command) -> Self {
        Self {
            apply: Some(cmd),
            side_effect: None,
            status: None,
            follow: Follow::Stay,
        }
    }
    pub fn apply_and_close(cmd: Command) -> Self {
        Self {
            apply: Some(cmd),
            side_effect: None,
            status: None,
            follow: Follow::Close,
        }
    }
    pub fn open(d: Box<dyn Dialog>) -> Self {
        Self {
            apply: None,
            side_effect: None,
            status: None,
            follow: Follow::Open(d),
        }
    }
    pub fn side_effect(eff: DialogSideEffect) -> Self {
        Self {
            apply: None,
            side_effect: Some(eff),
            status: None,
            follow: Follow::Stay,
        }
    }
    pub fn with_status(mut self, s: String) -> Self {
        self.status = Some(s);
        self
    }
    pub fn with_close(mut self) -> Self {
        self.follow = Follow::Close;
        self
    }
    pub fn with_close_to(mut self, mode: crate::app::AppMode) -> Self {
        self.follow = Follow::CloseTo(mode);
        self
    }
}

/// Direct side effects a dialog can request when the operation can't be
/// expressed as a `Command` (e.g. hard-deleting an archived board file).
pub enum DialogSideEffect {
    /// Hard-delete an archived board's directory.
    DeleteArchivedBoard { board_id: crate::model::ids::ShortId },
    /// Hard-delete an archived list and all its cards.
    DeleteArchivedList {
        list_id: crate::model::ids::ShortId,
        card_ids: Vec<crate::model::ids::ShortId>,
    },
    /// Hard-delete an archived card.
    DeleteArchivedCard { card_id: crate::model::ids::ShortId },
    /// Restore an archived board: apply the RestoreBoard command via a
    /// fresh editor (board is not loaded) AND re-add to board order.
    RestoreArchivedBoard { board_id: crate::model::ids::ShortId },
    /// Archive the currently-selected board (in Board Selector context):
    /// load editor for that board, apply ArchiveBoard, remove from order.
    ArchiveSelectedBoard,
    /// Stage an archived card into the loaded board's cache before
    /// applying RestoreCard. Used by the archived-cards dialog.
    StageAndRestoreCard { card: crate::model::card::Card },
    /// Discard the in-progress description edit and return to the
    /// previous mode. Used by ConfirmCancelEdit on `Yes`.
    DiscardDescriptionEdit,
    /// Resume the in-progress description edit (close cancel dialog,
    /// go back to Insert mode).
    ResumeDescriptionEdit,
    /// Reorder labels in the board's label palette (direct mutation +
    /// persist; not a `Command` per ADR-0002 — label reorder is tied to
    /// UI selection).
    ReorderLabels {
        from: usize,
        to: usize,
    },
    /// Start an Insert handler for a new label name. Opens the
    /// Insert mode; the resulting label will trigger a follow-up
    /// LabelManager dialog on confirm.
    StartNewLabelInsert,
    /// Start an Insert handler for renaming a label at the given index.
    StartRenameLabelInsert {
        label_idx: usize,
        current_name: String,
    },
}

/// A modal dialog with render and input handling.
pub trait Dialog {
    /// Draw the dialog. The dialog reads (read-only) state from the
    /// [`LoadedBoard`] when relevant; some dialogs (e.g. archived-boards)
    /// don't need it and may receive a `None` from the caller — those
    /// dialogs render purely from their own state.
    fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        board: Option<&LoadedBoard>,
        accent: Color,
    );

    /// Process a key event. The dialog may read the loaded board to
    /// resolve current selections (e.g. ConfirmArchiveCard looks up
    /// the currently selected card id at confirm time).
    fn handle_key(&mut self, key: KeyEvent, board: Option<&LoadedBoard>) -> DialogOutcome;

    /// Background layer this dialog overlays. Default: `Auto`.
    fn background(&self) -> DialogBackground {
        DialogBackground::Auto
    }
}
