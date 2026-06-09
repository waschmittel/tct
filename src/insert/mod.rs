//! Insert mode handlers. Each insert target is a struct implementing
//! [`InsertHandler`]; the active handler lives on `App.insert`.
//!
//! Adding a new insert target: create the struct in the appropriate
//! widget-kind submodule:
//!
//! - `line_editor` for single-line text input (titles, names, items)
//! - `markdown_editor` for the multi-line description editor
//! - `date_picker` for the due-date picker
//!
//! Each handler returns an [`InsertOutcome`] (`Stay`, `Cancel`,
//! `Confirm(Command)`) that the dispatcher interprets.

use crossterm::event::KeyEvent;

use crate::app::LoadedBoard;
use crate::command::Command;

pub mod date_picker;
pub mod line_editor;
pub mod line_input;
pub mod markdown_editor;
pub mod text_area_input;

/// Where in the UI an insert handler renders (drives background layer
/// selection in `ui::render`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertSurface {
    /// Inline popup over the **Board View** (most inserts).
    BoardView,
    /// Inline popup over the **Board Selector** (board name inserts).
    BoardSelector,
    /// Renders inside the open **Card Detail** popup.
    CardDetail,
}

/// Outcome of dispatching a key event to an insert handler.
pub enum InsertOutcome {
    /// Handler stays open with possibly updated state.
    Stay,
    /// Cancel insert; the dispatcher restores `previous_mode`.
    Cancel,
    /// Apply this command and close insert.
    Confirm(Command),
    /// Apply this command, close insert, then open a follow-up dialog.
    ConfirmAndOpenDialog(Command, Box<dyn crate::dialog::Dialog>),
    /// Close insert (no command) then open a dialog.
    OpenDialog(Box<dyn crate::dialog::Dialog>),
    /// Close insert with a status message but no command.
    CancelWithStatus(String),
    /// Confirm via a generic side effect (e.g. board create — not
    /// expressible as `Command` since the board isn't loaded).
    ConfirmSideEffect(Box<InsertSideEffect>),
}

/// Direct side effects an insert handler can request when the operation
/// isn't expressible as a `Command` (e.g. creating a new board file).
pub enum InsertSideEffect {
    /// Create a new board on disk with auto-pastel accent color.
    CreateBoard { name: String },
    /// Rename the board at the current selector index.
    RenameSelectedBoard { new_name: String },
}

/// A modal insert handler.
///
/// Rendering is read-only and is performed by `ui::board_view`,
/// `ui::board_selector`, or `ui::card_detail` by introspecting the
/// handler via [`InsertHandler::surface`], [`InsertHandler::title`],
/// [`InsertHandler::line_buffer`] / [`InsertHandler::line_cursor`], or
/// downcasting via [`InsertHandler::as_any`]. The trait exposes no
/// `render` method to keep handlers `Send`-friendly and rendering
/// concerns confined to `ui::`.
pub trait InsertHandler {
    /// Process a key event.
    fn handle_key(
        &mut self,
        key: KeyEvent,
        board: Option<&LoadedBoard>,
    ) -> InsertOutcome;

    /// What background surface this handler overlays.
    fn surface(&self) -> InsertSurface;

    /// Optional title for the insert popup (for line editors etc.).
    fn title(&self) -> &str {
        ""
    }

    /// Read access to the single-line buffer when present. Returns
    /// `None` for handlers that don't use a line buffer (markdown
    /// editor, date picker).
    fn line_buffer(&self) -> Option<&str> {
        None
    }

    /// Read access to the single-line cursor position (byte offset).
    fn line_cursor(&self) -> Option<usize> {
        None
    }

    /// Down-cast support for tests.
    fn as_any(&self) -> &dyn std::any::Any;
}
