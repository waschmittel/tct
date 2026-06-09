//! Archived Cards dialog — lists archived cards for the current board
//! and lets the user restore or hard-delete them.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use super::{Dialog, DialogOutcome, DialogSideEffect};
use crate::app::LoadedBoard;
use crate::model::card::Card;

pub struct ArchivedCards {
    pub cards: Vec<Card>,
    pub selected: usize,
}

impl Dialog for ArchivedCards {
    fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        _board: Option<&LoadedBoard>,
        accent: Color,
    ) {
        let height = (self.cards.len() as u16 + 4)
            .min(area.height.saturating_sub(4))
            .max(6);
        let width = 50u16.min(area.width.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let popup = Rect::new(x, y, width, height);

        frame.render_widget(Clear, popup);

        let block = Block::default()
            .title(" Archived Cards ")
            .title_bottom(Line::from(vec![
                Span::styled(" Enter", Style::default().fg(Color::Green)),
                Span::raw(":restore  "),
                Span::styled("x", Style::default().fg(Color::Red)),
                Span::raw(":delete  "),
                Span::styled("Esc", Style::default().fg(Color::Yellow)),
                Span::raw(":close "),
            ]))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));
        let inner = block.inner(popup);
        frame.render_widget(block, popup);

        if self.cards.is_empty() {
            frame.render_widget(
                Paragraph::new(Span::styled(
                    "No archived cards",
                    Style::default().fg(Color::DarkGray),
                )),
                inner,
            );
            return;
        }

        let mut lines = Vec::new();
        for (i, card) in self.cards.iter().enumerate() {
            let is_selected = i == self.selected;
            let prefix = if is_selected { "» " } else { "  " };
            let date = card.updated_at.format("%Y-%m-%d");
            let style = if is_selected {
                Style::default().fg(accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(
                format!("{prefix}{} ({})", card.title, date),
                style,
            )));
        }

        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn handle_key(&mut self, key: KeyEvent, _board: Option<&LoadedBoard>) -> DialogOutcome {
        match key.code {
            KeyCode::Down if self.selected < self.cards.len().saturating_sub(1) => {
                self.selected += 1;
                DialogOutcome::stay()
            }
            KeyCode::Up if self.selected > 0 => {
                self.selected -= 1;
                DialogOutcome::stay()
            }
            KeyCode::Enter if self.selected < self.cards.len() => {
                let card = self.cards.remove(self.selected);
                let title = card.title.clone();
                if self.selected > 0 && self.selected >= self.cards.len() {
                    self.selected = self.cards.len().saturating_sub(1);
                }
                let mut out =
                    DialogOutcome::side_effect(DialogSideEffect::StageAndRestoreCard { card })
                        .with_status(format!("Restored '{title}'"));
                if self.cards.is_empty() {
                    out = out.with_close();
                }
                out
            }
            KeyCode::Char('x') if self.selected < self.cards.len() => {
                let card = self.cards.remove(self.selected);
                let title = card.title.clone();
                if self.selected > 0 && self.selected >= self.cards.len() {
                    self.selected = self.cards.len().saturating_sub(1);
                }
                let mut out = DialogOutcome::side_effect(
                    DialogSideEffect::DeleteArchivedCard { card_id: card.id },
                )
                .with_status(format!("Deleted '{title}'"));
                if self.cards.is_empty() {
                    out = out.with_close();
                }
                out
            }
            KeyCode::Esc => DialogOutcome::close(),
            _ => DialogOutcome::stay(),
        }
    }
}
