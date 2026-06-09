//! Archived Boards dialog — restore or hard-delete archived boards.
//! Renders over the **Board Selector** background.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use super::{Dialog, DialogBackground, DialogOutcome, DialogSideEffect};
use crate::app::LoadedBoard;
use crate::model::board::BoardMeta;

pub struct ArchivedBoards {
    pub boards: Vec<BoardMeta>,
    pub selected: usize,
}

impl Dialog for ArchivedBoards {
    fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        _board: Option<&LoadedBoard>,
        _accent: Color,
    ) {
        let height = (self.boards.len() as u16 + 5)
            .min(area.height.saturating_sub(4))
            .max(6);
        let width = 54u16.min(area.width.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let popup = Rect::new(x, y, width, height);

        frame.render_widget(Clear, popup);

        let block = Block::default()
            .title(" Archived Boards ")
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

        if self.boards.is_empty() {
            frame.render_widget(
                Paragraph::new(Span::styled(
                    "No archived boards",
                    Style::default().fg(Color::DarkGray),
                )),
                inner,
            );
            return;
        }

        let mut lines = Vec::new();
        for (i, board) in self.boards.iter().enumerate() {
            let is_selected = i == self.selected;
            let prefix = if is_selected { "» " } else { "  " };
            let board_color = board.accent_color.to_ratatui_color();
            let style = if is_selected {
                Style::default().fg(board_color).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(board_color)
            };
            lines.push(Line::from(Span::styled(
                format!("{prefix}{}", board.name),
                style,
            )));
        }

        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn handle_key(&mut self, key: KeyEvent, _board: Option<&LoadedBoard>) -> DialogOutcome {
        match key.code {
            KeyCode::Down if self.selected < self.boards.len().saturating_sub(1) => {
                self.selected += 1;
                DialogOutcome::stay()
            }
            KeyCode::Up if self.selected > 0 => {
                self.selected -= 1;
                DialogOutcome::stay()
            }
            KeyCode::Enter if self.selected < self.boards.len() => {
                let board = self.boards.remove(self.selected);
                let name = board.name.clone();
                let board_id = board.id.clone();
                if self.selected > 0 && self.selected >= self.boards.len() {
                    self.selected = self.boards.len().saturating_sub(1);
                }
                let mut out = DialogOutcome::side_effect(
                    DialogSideEffect::RestoreArchivedBoard { board_id },
                )
                .with_status(format!("Restored board '{name}'"));
                if self.boards.is_empty() {
                    out = out.with_close_to(crate::app::AppMode::BoardSelector);
                }
                out
            }
            KeyCode::Char('x') if self.selected < self.boards.len() => {
                let board = self.boards.remove(self.selected);
                let name = board.name.clone();
                if self.selected > 0 && self.selected >= self.boards.len() {
                    self.selected = self.boards.len().saturating_sub(1);
                }
                let mut out = DialogOutcome::side_effect(
                    DialogSideEffect::DeleteArchivedBoard {
                        board_id: board.id,
                    },
                )
                .with_status(format!("Deleted board '{name}'"));
                if self.boards.is_empty() {
                    out = out.with_close_to(crate::app::AppMode::BoardSelector);
                }
                out
            }
            KeyCode::Esc => DialogOutcome::close_to(crate::app::AppMode::BoardSelector),
            _ => DialogOutcome::stay(),
        }
    }

    fn background(&self) -> DialogBackground {
        DialogBackground::BoardSelector
    }
}
