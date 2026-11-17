//! Confirmation dialog before renaming a label onto another label's name,
//! which merges the two. Reopens the Label Manager either way.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;

use super::label_manager::LabelManager;
use super::{Dialog, DialogOutcome, Follow};
use crate::app::LoadedBoard;
use crate::command::Command;
use crate::model::id::LabelId;

pub struct ConfirmMergeLabel {
    pub label_id: LabelId,
    /// Index of the renamed label in the manager, restored on cancel.
    pub label_idx: usize,
    /// The name it was renamed to, i.e. the surviving label's.
    pub name: String,
    pub from_picker: bool,
}

impl ConfirmMergeLabel {
    fn manager(&self, selected_idx: usize) -> Box<LabelManager> {
        Box::new(LabelManager {
            selected_idx,
            from_picker: self.from_picker,
        })
    }
}

impl Dialog for ConfirmMergeLabel {
    fn render(&self, frame: &mut Frame, area: Rect, board: Option<&LoadedBoard>, _accent: Color) {
        let old = board
            .and_then(|b| b.meta.labels.iter().find(|l| l.id == self.label_id))
            .map_or("this label", |l| l.name.as_str());
        super::common::render_confirm(
            frame,
            area,
            "Merge Labels",
            &format!("Merge '{old}' into '{}'?", self.name),
        );
    }

    fn handle_key(&mut self, key: KeyEvent, board: Option<&LoadedBoard>) -> DialogOutcome {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                // Select the survivor, whose index shifts if it sat below the removed label.
                let survivor = board
                    .and_then(|b| {
                        b.meta
                            .labels
                            .iter()
                            .position(|l| l.id != self.label_id && l.name == self.name)
                    })
                    .map_or(0, |i| if i > self.label_idx { i - 1 } else { i });
                let mut out = DialogOutcome::apply(Command::RenameLabel {
                    label_id: self.label_id.clone(),
                    name: self.name.clone(),
                })
                .with_status(format!("Merged into label '{}'", self.name));
                out.follow = Follow::Open(self.manager(survivor));
                out
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                DialogOutcome::open(self.manager(self.label_idx))
            }
            KeyCode::Char('?') => DialogOutcome::help(),
            _ => DialogOutcome::stay(),
        }
    }

    fn help(&self) -> Option<super::DialogHelp> {
        Some(super::DialogHelp {
            title: " Help — Merge Labels ",
            rows: vec![("y", "merge the labels"), ("n / esc", "cancel")],
        })
    }
}
