//! Read-only history viewer for the currently selected card.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use super::{Dialog, DialogOutcome};
use crate::app::LoadedBoard;

pub struct CardHistory {
    pub scroll: usize,
}

impl Dialog for CardHistory {
    fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        board: Option<&LoadedBoard>,
        accent: Color,
    ) {
        let card = match board.and_then(|b| b.current_card()) {
            Some(c) => c,
            None => return,
        };

        let local_fmt = |dt: chrono::DateTime<chrono::Utc>| {
            dt.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        };

        let mut entries: Vec<(String, String)> = Vec::new();
        entries.push(("Created".into(), local_fmt(card.created_at)));
        for entry in card.history.iter().rev() {
            entries.push((entry.action.clone(), local_fmt(entry.at)));
        }

        let width = 70u16.min(area.width.saturating_sub(4)).max(40);
        let height = ((entries.len() as u16).saturating_add(4))
            .min(area.height.saturating_sub(4))
            .max(8);
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let popup = Rect::new(x, y, width, height);

        frame.render_widget(Clear, popup);

        let title = format!(" History — {} ", truncate_for_title(&card.title, 40));
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent));
        let inner = block.inner(popup);
        frame.render_widget(block, popup);

        let visible_rows = inner.height as usize;
        let max_scroll = entries.len().saturating_sub(visible_rows);
        let scroll = self.scroll.min(max_scroll);

        let stamp_width = 16usize;
        let mut lines = Vec::with_capacity(visible_rows);
        for (action, stamp) in entries.iter().skip(scroll).take(visible_rows) {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{stamp:<stamp_width$}  "),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(action.clone(), Style::default().fg(Color::White)),
            ]));
        }

        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn handle_key(&mut self, key: KeyEvent, _board: Option<&LoadedBoard>) -> DialogOutcome {
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll = self.scroll.saturating_add(1);
                DialogOutcome::stay()
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.scroll = self.scroll.saturating_sub(1);
                DialogOutcome::stay()
            }
            KeyCode::PageDown => {
                self.scroll = self.scroll.saturating_add(10);
                DialogOutcome::stay()
            }
            KeyCode::PageUp => {
                self.scroll = self.scroll.saturating_sub(10);
                DialogOutcome::stay()
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.scroll = 0;
                DialogOutcome::stay()
            }
            KeyCode::Char('?') => DialogOutcome::help(),
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('h') => DialogOutcome::close(),
            _ => DialogOutcome::stay(),
        }
    }

    fn help(&self) -> Option<super::DialogHelp> {
        Some(super::DialogHelp {
            title: " Help — Card History ",
            rows: vec![
                ("Up / Down / j / k", "Scroll"),
                ("PgUp / PgDn", "Scroll by 10"),
                ("g / Home", "Jump to top"),
                ("Esc / q / h", "Close"),
            ],
        })
    }
}

fn truncate_for_title(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}
