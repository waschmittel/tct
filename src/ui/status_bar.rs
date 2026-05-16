use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{App, AppMode};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let mode_str = match &app.mode {
        AppMode::BoardSelector => "BOARDS",
        AppMode::Normal => "NORMAL",
        AppMode::CardDetail => "DETAIL",
        AppMode::Insert(_) => "INSERT",
        AppMode::Command => "SEARCH",
        AppMode::Dialog(_) => "DIALOG",
        AppMode::Help => "HELP",
    };

    let hints = match &app.mode {
        AppMode::BoardSelector => "↑/↓:navigate  Shift+↑/↓:reorder  Enter:open  n:new  c:color  d:archive  v:archived  q:quit",
        AppMode::Normal => "←/→:lists  ↑/↓:cards  Enter:open  e:title  n:new  N:list  a:archive  v:unarchive  Shift+Arrow:move  y:copy  /:search  ?:help  q:quit",
        AppMode::CardDetail => "↑/↓:items  S+↑/↓:reorder  Space:toggle  a:add  x:del  Enter:edit  e:desc  t:title  y:copy  l:labels  u:due  Esc:close",
        AppMode::Insert(_) => "Enter:confirm  Esc:cancel  Ctrl+u:clear",
        AppMode::Command => "Enter:search  Esc:cancel",
        AppMode::Dialog(_) => "y:confirm  n:cancel",
        AppMode::Help => "Esc:close",
    };

    let status = if let Some((msg, _)) = &app.status_message {
        msg.as_str()
    } else {
        ""
    };

    let line1 = Line::from(vec![
        Span::styled(
            format!(" {mode_str} "),
            Style::default().fg(Color::Black).bg(app.accent_color()),
        ),
        Span::raw(" "),
        Span::styled(hints, Style::default().fg(Color::DarkGray)),
    ]);

    let line2 = if status.is_empty() {
        Line::raw("")
    } else {
        Line::from(Span::styled(
            format!(" {status}"),
            Style::default().fg(Color::Yellow),
        ))
    };

    let paragraph = Paragraph::new(vec![line1, line2]);
    frame.render_widget(paragraph, area);
}
