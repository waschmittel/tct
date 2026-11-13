//! Label Filter dialog — pick the label the board view is filtered by.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use super::{Dialog, DialogOutcome, DialogSideEffect};
use crate::app::LoadedBoard;

pub struct LabelFilterPicker {
    pub selected_idx: usize,
}

impl Dialog for LabelFilterPicker {
    fn render(&self, frame: &mut Frame, area: Rect, board: Option<&LoadedBoard>, accent: Color) {
        let Some(board) = board else { return };
        let labels = &board.meta.labels;

        let height = (labels.len() as u16 + 6)
            .min(area.height.saturating_sub(4))
            .max(6);
        let width = 36u16.min(area.width.saturating_sub(4));
        let popup = Rect::new(
            area.width.saturating_sub(width) / 2,
            area.height.saturating_sub(height) / 2,
            width,
            height,
        );
        frame.render_widget(Clear, popup);

        let block = Block::default()
            .title(" Filter by label ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent));
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

        let lines: Vec<Line> = labels
            .iter()
            .enumerate()
            .map(|(i, label)| {
                let active = board.label_filter.as_ref() == Some(&label.id);
                let mark = if active { "●" } else { "○" };
                let style = Style::default().fg(label.color.to_ratatui_color());
                if i == self.selected_idx {
                    Line::from(vec![
                        Span::styled(
                            "» ",
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!("{mark} {}", label.name),
                            style.add_modifier(Modifier::BOLD),
                        ),
                    ])
                } else {
                    Line::from(Span::styled(format!("  {mark} {}", label.name), style))
                }
            })
            .collect();
        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn handle_key(&mut self, key: KeyEvent, board: Option<&LoadedBoard>) -> DialogOutcome {
        let count = board.map(|b| b.meta.labels.len()).unwrap_or(0);
        match key.code {
            KeyCode::Char('?') => DialogOutcome::help(),
            KeyCode::Esc => DialogOutcome::close(),
            KeyCode::Down if self.selected_idx + 1 < count => {
                self.selected_idx += 1;
                DialogOutcome::stay()
            }
            KeyCode::Up if self.selected_idx > 0 => {
                self.selected_idx -= 1;
                DialogOutcome::stay()
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                let Some(board) = board else {
                    return DialogOutcome::stay();
                };
                let Some(label) = board.meta.labels.get(self.selected_idx) else {
                    return DialogOutcome::stay();
                };
                // Picking the active label again clears the filter.
                let label_id =
                    (board.label_filter.as_ref() != Some(&label.id)).then(|| label.id.clone());
                DialogOutcome::side_effect(DialogSideEffect::SetLabelFilter { label_id })
                    .with_close()
            }
            _ => DialogOutcome::stay(),
        }
    }

    fn help(&self) -> Option<super::DialogHelp> {
        Some(super::DialogHelp {
            title: " Help — Filter by label ",
            rows: vec![
                ("↑ / ↓", "select label"),
                (
                    "enter / space",
                    "show only cards with this label (again: clear)",
                ),
                ("esc", "back"),
            ],
        })
    }
}
