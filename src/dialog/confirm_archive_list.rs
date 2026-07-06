//! Confirmation dialog before archiving the selected list.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;

use super::{Dialog, DialogOutcome};
use crate::app::LoadedBoard;
use crate::command::Command;

pub struct ConfirmArchiveList;

impl Dialog for ConfirmArchiveList {
    fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        board: Option<&LoadedBoard>,
        _accent: Color,
    ) {
        let name = board
            .and_then(|b| b.lists.get(b.selected_list))
            .map(|l| l.name.as_str())
            .unwrap_or("this list");
        super::common::render_confirm(
            frame,
            area,
            "Archive List",
            &format!("Archive list '{name}'?"),
        );
    }

    fn handle_key(&mut self, key: KeyEvent, board: Option<&LoadedBoard>) -> DialogOutcome {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some((list_id, name)) = board
                    .and_then(|b| b.lists.get(b.selected_list))
                    .map(|l| (l.id.clone(), l.name.clone()))
                {
                    DialogOutcome::apply_and_close(Command::ArchiveList { list_id })
                        .with_status(format!("Archived list '{name}'"))
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
            title: " Help — Archive List ",
            rows: vec![("y", "Archive list"), ("n / Esc", "Cancel")],
        })
    }
}
