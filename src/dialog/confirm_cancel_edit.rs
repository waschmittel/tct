//! Discard-changes prompt shown when leaving the description editor
//! with unsaved changes.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;

use super::{Dialog, DialogOutcome, DialogSideEffect};
use crate::app::LoadedBoard;

pub struct ConfirmCancelEdit;

impl Dialog for ConfirmCancelEdit {
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
            "Discard Changes",
            "Discard unsaved changes?",
        );
    }

    fn handle_key(&mut self, key: KeyEvent, _board: Option<&LoadedBoard>) -> DialogOutcome {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                DialogOutcome::side_effect(DialogSideEffect::DiscardDescriptionEdit)
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                DialogOutcome::side_effect(DialogSideEffect::ResumeDescriptionEdit)
            }
            KeyCode::Char('?') => DialogOutcome::help(),
            _ => DialogOutcome::stay(),
        }
    }

    fn help(&self) -> Option<super::DialogHelp> {
        Some(super::DialogHelp {
            title: " Help — Discard Changes ",
            rows: vec![("y", "Discard changes"), ("n / Esc", "Keep editing")],
        })
    }
}
