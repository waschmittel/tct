//! Label Picker dialog — toggle which labels are assigned to the
//! currently selected card.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use super::{Dialog, DialogOutcome};
use crate::app::LoadedBoard;
use crate::command::Command;

pub struct LabelPicker {
    pub selected_idx: usize,
}

impl Dialog for LabelPicker {
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

        let board_labels = &board.meta.labels;
        let card_label_ids: Vec<_> = board
            .current_card()
            .map(|c| c.label_ids.as_slice())
            .unwrap_or(&[])
            .to_vec();

        let height = (board_labels.len() as u16 + 6)
            .min(area.height.saturating_sub(4))
            .max(6);
        let width = 36u16.min(area.width.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let popup = Rect::new(x, y, width, height);

        frame.render_widget(Clear, popup);

        let block = Block::default()
            .title(" Labels (Space:toggle, L:manage, ?:help) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent));
        let inner = block.inner(popup);
        frame.render_widget(block, popup);

        if board_labels.is_empty() {
            frame.render_widget(
                Paragraph::new(Span::styled(
                    "No labels. Press L to manage.",
                    Style::default().fg(Color::DarkGray),
                )),
                inner,
            );
            return;
        }

        let mut lines = vec![];
        for (i, label) in board_labels.iter().enumerate() {
            let assigned = card_label_ids.contains(&label.id);
            let is_selected = i == self.selected_idx;
            let check = if assigned { "●" } else { "○" };
            let label_style = Style::default().fg(label.color.to_ratatui_color());

            if is_selected {
                lines.push(Line::from(vec![
                    Span::styled(
                        "» ",
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{check} {}", label.name),
                        label_style.add_modifier(Modifier::BOLD),
                    ),
                ]));
            } else {
                lines.push(Line::from(Span::styled(
                    format!("  {check} {}", label.name),
                    label_style,
                )));
            }
        }

        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn handle_key(&mut self, key: KeyEvent, board: Option<&LoadedBoard>) -> DialogOutcome {
        let label_count = board.map(|b| b.meta.labels.len()).unwrap_or(0);

        if let KeyCode::Char('L') = key.code {
            return DialogOutcome::open(Box::new(super::label_manager::LabelManager {
                selected_idx: 0,
                from_picker: true,
            }));
        }
        if let KeyCode::Char('?') = key.code {
            return DialogOutcome::help();
        }

        if label_count == 0 {
            return match key.code {
                KeyCode::Esc => DialogOutcome::close(),
                _ => DialogOutcome::stay(),
            };
        }

        match key.code {
            KeyCode::Down if self.selected_idx < label_count - 1 => {
                self.selected_idx += 1;
                DialogOutcome::stay()
            }
            KeyCode::Up if self.selected_idx > 0 => {
                self.selected_idx -= 1;
                DialogOutcome::stay()
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(board) = board {
                    let label_id =
                        board.meta.labels.get(self.selected_idx).map(|l| l.id.clone());
                    let card_id = board.current_card_id().cloned();
                    if let (Some(lid), Some(cid)) = (label_id, card_id) {
                        return DialogOutcome::apply(Command::ToggleLabel {
                            card_id: cid,
                            label_id: lid,
                        });
                    }
                }
                DialogOutcome::stay()
            }
            KeyCode::Esc => DialogOutcome::close(),
            _ => DialogOutcome::stay(),
        }
    }

    fn help(&self) -> Option<super::DialogHelp> {
        Some(super::DialogHelp {
            title: " Help — Labels ",
            rows: vec![
                ("Up / Down", "Select label"),
                ("Space / Enter", "Toggle label on card"),
                ("L", "Manage labels (create, edit, delete)"),
                ("Esc", "Back"),
            ],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialog::Follow;
    use crossterm::event::{KeyEvent, KeyModifiers};

    #[test]
    fn uppercase_l_opens_label_manager_even_without_labels() {
        let mut picker = LabelPicker { selected_idx: 0 };
        let key = KeyEvent::new(KeyCode::Char('L'), KeyModifiers::SHIFT);
        let outcome = picker.handle_key(key, None);
        assert!(matches!(outcome.follow, Follow::Open(_)));
    }
}
