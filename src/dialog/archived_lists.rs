//! Archived Lists dialog — restore or hard-delete archived lists for
//! the current board.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use super::{Dialog, DialogOutcome, DialogSideEffect};
use crate::app::LoadedBoard;
use crate::command::Command;
use crate::model::list::CardList;

pub struct ArchivedLists {
    pub lists: Vec<CardList>,
    pub selected: usize,
}

impl Dialog for ArchivedLists {
    fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        _board: Option<&LoadedBoard>,
        accent: Color,
    ) {
        let height = (self.lists.len() as u16 + 5)
            .min(area.height.saturating_sub(4))
            .max(6);
        let width = 54u16.min(area.width.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let popup = Rect::new(x, y, width, height);

        frame.render_widget(Clear, popup);

        let block = Block::default()
            .title(" Archived Lists ")
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

        if self.lists.is_empty() {
            frame.render_widget(
                Paragraph::new(Span::styled(
                    "No archived lists",
                    Style::default().fg(Color::DarkGray),
                )),
                inner,
            );
            return;
        }

        let mut lines = Vec::new();
        for (i, list) in self.lists.iter().enumerate() {
            let is_selected = i == self.selected;
            let prefix = if is_selected { "» " } else { "  " };
            let style = if is_selected {
                Style::default().fg(accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(
                format!("{prefix}{} ({} cards)", list.name, list.card_ids.len()),
                style,
            )));
        }

        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn handle_key(&mut self, key: KeyEvent, _board: Option<&LoadedBoard>) -> DialogOutcome {
        match key.code {
            KeyCode::Down if self.selected < self.lists.len().saturating_sub(1) => {
                self.selected += 1;
                DialogOutcome::stay()
            }
            KeyCode::Up if self.selected > 0 => {
                self.selected -= 1;
                DialogOutcome::stay()
            }
            KeyCode::Enter if self.selected < self.lists.len() => {
                let list = self.lists.remove(self.selected);
                let name = list.name.clone();
                let list_id = list.id.clone();
                if self.selected > 0 && self.selected >= self.lists.len() {
                    self.selected = self.lists.len().saturating_sub(1);
                }
                let mut out = DialogOutcome::apply(Command::RestoreList { list_id })
                    .with_status(format!("Restored list '{name}'"));
                if self.lists.is_empty() {
                    out = out.with_close();
                }
                out
            }
            KeyCode::Char('x') if self.selected < self.lists.len() => {
                let list = self.lists.remove(self.selected);
                if self.selected > 0 && self.selected >= self.lists.len() {
                    self.selected = self.lists.len().saturating_sub(1);
                }
                let name = list.name.clone();
                let mut out =
                    DialogOutcome::side_effect(DialogSideEffect::DeleteArchivedList {
                        list_id: list.id,
                        card_ids: list.card_ids,
                    })
                    .with_status(format!("Deleted list '{name}'"));
                if self.lists.is_empty() {
                    out = out.with_close();
                }
                out
            }
            KeyCode::Esc => DialogOutcome::close(),
            _ => DialogOutcome::stay(),
        }
    }
}
