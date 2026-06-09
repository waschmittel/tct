//! Shared rendering helpers used by multiple dialogs.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

/// Centered yes/no confirmation popup with a red border.
pub fn render_confirm(frame: &mut Frame, area: Rect, title: &str, message: &str) {
    let width = 50u16.min(area.width.saturating_sub(4));
    let height = 6u16;
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(format!(" {title} "))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let lines = vec![
        Line::from(Span::styled(message, Style::default().fg(Color::White))),
        Line::raw(""),
        Line::from(vec![
            Span::styled(" y ", Style::default().fg(Color::Black).bg(Color::Red)),
            Span::raw(" Yes    "),
            Span::styled(" n ", Style::default().fg(Color::Black).bg(Color::Green)),
            Span::raw(" No"),
        ]),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
}
