//! Notice shown when a card is requested on a board without any list.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use super::{Dialog, DialogOutcome, DialogSideEffect};
use crate::app::LoadedBoard;

/// Cards live in lists, so there is nothing to add a card to yet. Offers to
/// create the first list instead of opening a card dialog that cannot work.
pub struct NoLists;

impl Dialog for NoLists {
    fn render(&self, frame: &mut Frame, area: Rect, _board: Option<&LoadedBoard>, _accent: Color) {
        let width = 52u16.min(area.width.saturating_sub(4));
        let height = 8u16;
        let popup = Rect::new(
            area.width.saturating_sub(width) / 2,
            area.height.saturating_sub(height) / 2,
            width,
            height,
        );
        frame.render_widget(Clear, popup);

        let block = Block::default()
            .title(" Can't add a card ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red));
        let inner = block.inner(popup);
        frame.render_widget(block, popup);

        let lines = vec![
            Line::from(Span::styled(
                "This board has no lists yet.",
                Style::default().fg(Color::Red),
            )),
            Line::raw(""),
            Line::raw("Cards live in lists. Create a list first,"),
            Line::raw("then add cards to it."),
            Line::raw(""),
            Line::from(vec![
                Span::styled(" N ", Style::default().fg(Color::Black).bg(Color::Green)),
                Span::raw(" Create a list    "),
                Span::styled(" Esc ", Style::default().fg(Color::Black).bg(Color::Gray)),
                Span::raw(" Close"),
            ]),
        ];
        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn handle_key(&mut self, key: KeyEvent, _board: Option<&LoadedBoard>) -> DialogOutcome {
        match key.code {
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Enter => {
                DialogOutcome::side_effect(DialogSideEffect::StartNewListInsert)
            }
            KeyCode::Esc | KeyCode::Char('q') => DialogOutcome::close(),
            KeyCode::Char('?') => DialogOutcome::help(),
            _ => DialogOutcome::stay(),
        }
    }

    fn help(&self) -> Option<super::DialogHelp> {
        Some(super::DialogHelp {
            title: " Help — No Lists ",
            rows: vec![("n / enter", "create a list"), ("esc / q", "close")],
        })
    }
}
