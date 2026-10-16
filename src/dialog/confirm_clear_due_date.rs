//! Confirmation dialog before clearing the selected card's due date.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;

use super::{Dialog, DialogOutcome};
use crate::app::LoadedBoard;
use crate::command::Command;

/// Holds no payload — resolves the current card id from the loaded board
/// at confirm time, like [`super::confirm_archive_card::ConfirmArchiveCard`].
pub struct ConfirmClearDueDate;

impl Dialog for ConfirmClearDueDate {
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
        super::common::render_confirm(
            frame,
            area,
            "Clear Due Date",
            &format!("Clear due date of '{name}'?"),
        );
    }

    fn handle_key(&mut self, key: KeyEvent, board: Option<&LoadedBoard>) -> DialogOutcome {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some((card_id, title)) = board
                    .and_then(|b| b.current_card())
                    .map(|c| (c.id.clone(), c.title.clone()))
                {
                    DialogOutcome::apply_and_close(Command::ClearDueDate { card_id })
                        .with_status(format!("Cleared due date of '{title}'"))
                } else {
                    DialogOutcome::close()
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => DialogOutcome::close(),
            KeyCode::Char('?') => DialogOutcome::help(),
            _ => DialogOutcome::stay(),
        }
    }

    fn help(&self) -> Option<super::DialogHelp> {
        Some(super::DialogHelp {
            title: " Help — Clear Due Date ",
            rows: vec![("y", "Clear due date"), ("n / Esc", "Cancel")],
        })
    }
}
