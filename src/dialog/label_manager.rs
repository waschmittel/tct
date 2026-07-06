//! Label Manager dialog — create, rename, recolor, reorder, or delete
//! labels in the board's label palette.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use super::{Dialog, DialogOutcome, DialogSideEffect};
use crate::app::LoadedBoard;
use crate::command::Command;

pub struct LabelManager {
    pub selected_idx: usize,
    /// Opened from the LabelPicker — closing returns to the picker
    /// instead of leaving Dialog mode.
    pub from_picker: bool,
}

impl Dialog for LabelManager {
    fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        board: Option<&LoadedBoard>,
        _accent: Color,
    ) {
        let board = match board {
            Some(b) => b,
            None => return,
        };

        let labels = &board.meta.labels;
        let height = (labels.len() as u16 + 6)
            .min(area.height.saturating_sub(4))
            .max(8);
        let width = 40u16.min(area.width.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let popup = Rect::new(x, y, width, height);

        frame.render_widget(Clear, popup);

        let block = Block::default()
            .title(" Label Manager ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));
        let inner = block.inner(popup);
        frame.render_widget(block, popup);

        if labels.is_empty() {
            frame.render_widget(
                Paragraph::new(Span::styled(
                    "No labels.",
                    Style::default().fg(Color::DarkGray),
                )),
                inner,
            );
            return;
        }

        let mut lines = vec![];
        for (i, label) in labels.iter().enumerate() {
            let is_selected = i == self.selected_idx;
            let label_style = Style::default()
                .fg(Color::Black)
                .bg(label.color.to_ratatui_color());

            if is_selected {
                lines.push(Line::from(vec![
                    Span::styled(
                        "» ",
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(" {} ", label.name),
                        label_style.add_modifier(Modifier::BOLD),
                    ),
                ]));
            } else {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(format!(" {} ", label.name), label_style),
                ]));
            }
        }

        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn handle_key(&mut self, key: KeyEvent, board: Option<&LoadedBoard>) -> DialogOutcome {
        let label_count = board.map(|b| b.meta.labels.len()).unwrap_or(0);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);

        match (key.code, shift) {
            (KeyCode::Down, false)
                if label_count > 0 && self.selected_idx < label_count - 1 =>
            {
                self.selected_idx += 1;
                DialogOutcome::stay()
            }
            (KeyCode::Up, false) if self.selected_idx > 0 => {
                self.selected_idx -= 1;
                DialogOutcome::stay()
            }
            (KeyCode::Down, true) => {
                if self.selected_idx + 1 < label_count {
                    let from = self.selected_idx;
                    self.selected_idx += 1;
                    DialogOutcome::side_effect(DialogSideEffect::ReorderLabels {
                        from,
                        to: from + 1,
                    })
                } else {
                    DialogOutcome::stay()
                }
            }
            (KeyCode::Up, true) => {
                if self.selected_idx > 0 {
                    let from = self.selected_idx;
                    self.selected_idx -= 1;
                    DialogOutcome::side_effect(DialogSideEffect::ReorderLabels {
                        from,
                        to: from - 1,
                    })
                } else {
                    DialogOutcome::stay()
                }
            }
            (KeyCode::Char('n'), _) => {
                DialogOutcome::side_effect(DialogSideEffect::StartNewLabelInsert {
                    from_picker: self.from_picker,
                })
            }
            (KeyCode::Char('e'), _) => {
                if label_count > 0
                    && let Some(board) = board
                    && let Some(label) = board.meta.labels.get(self.selected_idx)
                {
                    DialogOutcome::side_effect(DialogSideEffect::StartRenameLabelInsert {
                        label_idx: self.selected_idx,
                        current_name: label.name.clone(),
                        from_picker: self.from_picker,
                    })
                } else {
                    DialogOutcome::stay()
                }
            }
            (KeyCode::Char('c'), _) => {
                if label_count > 0 {
                    let label_color = board
                        .and_then(|b| b.meta.labels.get(self.selected_idx))
                        .map(|l| (l.id.clone(), l.color.next()));
                    if let Some((label_id, color)) = label_color {
                        return DialogOutcome::apply(Command::SetLabelColor { label_id, color });
                    }
                }
                DialogOutcome::stay()
            }
            (KeyCode::Char('C'), _) => {
                let label = board.and_then(|b| b.meta.labels.get(self.selected_idx));
                if let Some(label) = label {
                    DialogOutcome::open(Box::new(super::color_picker::ColorPicker::for_label(
                        label.color,
                        label.id.clone(),
                        self.selected_idx,
                        self.from_picker,
                    )))
                } else {
                    DialogOutcome::stay()
                }
            }
            (KeyCode::Char('d'), _) if label_count > 0 => DialogOutcome::open(Box::new(
                super::confirm_delete_label::ConfirmDeleteLabel {
                    label_idx: self.selected_idx,
                    from_picker: self.from_picker,
                },
            )),
            (KeyCode::Char('?'), _) => DialogOutcome::help(),
            (KeyCode::Esc, _) => {
                if self.from_picker {
                    DialogOutcome::open(Box::new(super::label_picker::LabelPicker {
                        selected_idx: 0,
                    }))
                } else {
                    DialogOutcome::close()
                }
            }
            _ => DialogOutcome::stay(),
        }
    }

    fn help(&self) -> Option<super::DialogHelp> {
        Some(super::DialogHelp {
            title: " Help — Label Manager ",
            rows: vec![
                ("Up / Down", "Select label"),
                ("Shift+Up/Down", "Reorder label"),
                ("n", "New label"),
                ("e", "Rename label"),
                ("c", "Cycle preset color"),
                ("C", "Pick color (HSL)"),
                ("d", "Delete label"),
                ("Esc", "Close"),
            ],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialog::Follow;
    use crate::model::board::BoardMeta;
    use crate::model::label::{Label, LabelColor};
    use std::collections::HashMap;

    fn board_with_label() -> LoadedBoard {
        let mut meta = BoardMeta::new("Test".into());
        meta.labels.push(Label::new("bug".into(), LabelColor::Red));
        LoadedBoard {
            meta,
            lists: vec![],
            cards: HashMap::new(),
            selected_list: 0,
            selected_card: vec![],
            scroll_offset: vec![],
            detail_item_idx: 0,
            detail_scroll: 0,
        }
    }

    fn press(mgr: &mut LabelManager, code: KeyCode, board: Option<&LoadedBoard>) -> DialogOutcome {
        mgr.handle_key(KeyEvent::new(code, KeyModifiers::empty()), board)
    }

    #[test]
    fn esc_from_picker_reopens_picker() {
        let mut mgr = LabelManager { selected_idx: 0, from_picker: true };
        let outcome = press(&mut mgr, KeyCode::Esc, None);
        assert!(matches!(outcome.follow, Follow::Open(_)));
    }

    #[test]
    fn esc_without_picker_origin_closes_and_restores_origin_mode() {
        let mut mgr = LabelManager { selected_idx: 0, from_picker: false };
        let outcome = press(&mut mgr, KeyCode::Esc, None);
        assert!(matches!(outcome.follow, Follow::Close));
    }

    #[test]
    fn d_opens_delete_confirmation() {
        let board = board_with_label();
        let mut mgr = LabelManager { selected_idx: 0, from_picker: false };
        let outcome = press(&mut mgr, KeyCode::Char('d'), Some(&board));
        assert!(matches!(outcome.follow, Follow::Open(_)));
    }

    #[test]
    fn x_is_not_bound() {
        let board = board_with_label();
        let mut mgr = LabelManager { selected_idx: 0, from_picker: false };
        let outcome = press(&mut mgr, KeyCode::Char('x'), Some(&board));
        assert!(matches!(outcome.follow, Follow::Stay));
    }

    #[test]
    fn question_mark_shows_dialog_help() {
        let mut mgr = LabelManager { selected_idx: 0, from_picker: false };
        let outcome = press(&mut mgr, KeyCode::Char('?'), None);
        assert!(matches!(outcome.follow, Follow::Help));
        assert!(mgr.help().is_some());
    }

    #[test]
    fn capital_c_opens_color_picker_for_selected_label() {
        let board = board_with_label();
        let mut mgr = LabelManager { selected_idx: 0, from_picker: false };
        let outcome = press(&mut mgr, KeyCode::Char('C'), Some(&board));
        assert!(matches!(outcome.follow, Follow::Open(_)));
    }
}
