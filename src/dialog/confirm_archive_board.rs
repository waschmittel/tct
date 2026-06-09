//! Confirmation dialog before archiving the selected board from the
//! Board Selector. Uses the **Board Selector** as background layer.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;

use super::{Dialog, DialogBackground, DialogOutcome, DialogSideEffect};
use crate::app::LoadedBoard;

/// Carries the board name for the prompt, since the loaded board may
/// not be `Some` (the user is in Board Selector).
pub struct ConfirmArchiveBoard {
    pub board_name: String,
}

impl Dialog for ConfirmArchiveBoard {
    fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        _board: Option<&LoadedBoard>,
        _accent: Color,
    ) {
        super::common::render_confirm(
            frame,
            area,
            "Archive Board",
            &format!("Archive board '{}'?", self.board_name),
        );
    }

    fn handle_key(&mut self, key: KeyEvent, _board: Option<&LoadedBoard>) -> DialogOutcome {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                DialogOutcome::side_effect(DialogSideEffect::ArchiveSelectedBoard)
                    .with_close_to(crate::app::AppMode::BoardSelector)
                    .with_status("Board archived".into())
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                DialogOutcome::close_to(crate::app::AppMode::BoardSelector)
            }
            _ => DialogOutcome::stay(),
        }
    }

    fn background(&self) -> DialogBackground {
        DialogBackground::BoardSelector
    }
}
