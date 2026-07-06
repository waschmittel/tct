//! Confirmation dialog before hard-deleting a label from the
//! Label Manager. On confirmation (or cancellation), reopens the
//! Label Manager dialog.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;

use super::{Dialog, DialogOutcome};
use crate::app::LoadedBoard;
use crate::command::Command;

pub struct ConfirmDeleteLabel {
    /// Index of the label in the board's label palette, used to look up
    /// the label at confirm time so the dialog stays in sync with any
    /// reorders happening underneath.
    pub label_idx: usize,
    /// Threaded through from the LabelManager so the reopened manager
    /// still returns to the LabelPicker on close.
    pub from_picker: bool,
}

impl Dialog for ConfirmDeleteLabel {
    fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        board: Option<&LoadedBoard>,
        _accent: Color,
    ) {
        let name = board
            .and_then(|b| b.meta.labels.get(self.label_idx))
            .map(|l| l.name.as_str())
            .unwrap_or("this label");
        super::common::render_confirm(
            frame,
            area,
            "Delete Label",
            &format!("Delete '{name}' from all cards?"),
        );
    }

    fn handle_key(&mut self, key: KeyEvent, board: Option<&LoadedBoard>) -> DialogOutcome {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let target = board.and_then(|b| {
                    b.meta
                        .labels
                        .get(self.label_idx)
                        .map(|l| (l.id.clone(), l.name.clone()))
                });
                if let Some((label_id, name)) = target {
                    let mut out = DialogOutcome::apply(Command::DeleteLabel { label_id })
                        .with_status(format!("Deleted label '{name}'"));
                    out.follow = super::Follow::Open(Box::new(super::label_manager::LabelManager {
                        selected_idx: self.label_idx,
                        from_picker: self.from_picker,
                    }));
                    out
                } else {
                    DialogOutcome::open(Box::new(super::label_manager::LabelManager {
                        selected_idx: self.label_idx,
                        from_picker: self.from_picker,
                    }))
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                DialogOutcome::open(Box::new(super::label_manager::LabelManager {
                    selected_idx: self.label_idx,
                    from_picker: self.from_picker,
                }))
            }
            KeyCode::Char('?') => DialogOutcome::help(),
            _ => DialogOutcome::stay(),
        }
    }

    fn help(&self) -> Option<super::DialogHelp> {
        Some(super::DialogHelp {
            title: " Help — Delete Label ",
            rows: vec![("y", "Delete label from all cards"), ("n / Esc", "Cancel")],
        })
    }
}
