//! Confirmation dialog before archiving the selected card.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;

use super::{Dialog, DialogOutcome};
use crate::app::LoadedBoard;
use crate::command::Command;

/// Holds no payload — resolves the current card id from the loaded board
/// at confirm time so the command operates on whatever is selected when
/// the user presses **y**.
pub struct ConfirmArchiveCard;

impl Dialog for ConfirmArchiveCard {
    fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        board: Option<&LoadedBoard>,
        _accent: Color,
    ) {
        let name = board
            .and_then(|b| b.current_card())
            .map(|c| c.title.as_str())
            .unwrap_or("this card");
        super::common::render_confirm(frame, area, "Archive Card", &format!("Archive '{name}'?"));
    }

    fn handle_key(&mut self, key: KeyEvent, board: Option<&LoadedBoard>) -> DialogOutcome {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some((card_id, title)) = board
                    .and_then(|b| b.current_card())
                    .map(|c| (c.id.clone(), c.title.clone()))
                {
                    DialogOutcome::apply_and_close(Command::ArchiveCard { card_id })
                        .with_status(format!("Archived card '{title}'"))
                } else {
                    DialogOutcome::close()
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => DialogOutcome::close(),
            _ => DialogOutcome::stay(),
        }
    }
}
