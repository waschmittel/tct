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
}

impl Dialog for LabelManager {
    fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        board: Option<&LoadedBoard>,
        accent: Color,
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
            .title_bottom(Line::from(vec![
                Span::styled(" n", Style::default().fg(accent)),
                Span::raw(":new  "),
                Span::styled("e", Style::default().fg(accent)),
                Span::raw(":rename  "),
                Span::styled("c", Style::default().fg(accent)),
                Span::raw(":color  "),
                Span::styled("S+↑/↓", Style::default().fg(accent)),
                Span::raw(":reorder  "),
                Span::styled("x", Style::default().fg(accent)),
                Span::raw(":delete  "),
                Span::styled("Esc", Style::default().fg(Color::Yellow)),
                Span::raw(":close "),
            ]))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));
        let inner = block.inner(popup);
        frame.render_widget(block, popup);

        if labels.is_empty() {
            frame.render_widget(
                Paragraph::new(Span::styled(
                    "No labels. Press 'n' to create one.",
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
                DialogOutcome::side_effect(DialogSideEffect::StartNewLabelInsert)
            }
            (KeyCode::Char('e'), _) => {
                if label_count > 0
                    && let Some(board) = board
                    && let Some(label) = board.meta.labels.get(self.selected_idx)
                {
                    DialogOutcome::side_effect(DialogSideEffect::StartRenameLabelInsert {
                        label_idx: self.selected_idx,
                        current_name: label.name.clone(),
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
            (KeyCode::Char('x'), _) if label_count > 0 => DialogOutcome::open(Box::new(
                super::confirm_delete_label::ConfirmDeleteLabel {
                    label_idx: self.selected_idx,
                },
            )),
            (KeyCode::Esc, _) => DialogOutcome::close_to(crate::app::AppMode::Normal),
            _ => DialogOutcome::stay(),
        }
    }
}
