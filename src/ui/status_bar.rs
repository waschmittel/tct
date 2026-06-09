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
        AppMode::Insert => "INSERT",
        AppMode::Command => "SEARCH",
        AppMode::Dialog => "DIALOG",
        AppMode::Help => "HELP",
    };

    let insert_hint = if matches!(app.mode, AppMode::Insert) {
        app.insert.as_ref().map(|h| {
            let title = h.title();
            if h.as_any()
                .downcast_ref::<crate::insert::date_picker::DatePicker>()
                .is_some()
            {
                format!("{title}  ←→↑↓:navigate  t:today  Enter:save  Esc:cancel")
            } else if h.as_any()
                .downcast_ref::<crate::insert::markdown_editor::MarkdownEditor>()
                .is_some()
            {
                format!("{title}  Ctrl+S:save  Esc:cancel  Tab:nest")
            } else {
                format!("{title}  Enter:confirm  Esc:cancel")
            }
        })
    } else {
        None
    };

    let hints: &str = match &app.mode {
        AppMode::BoardSelector => "?:help  q:quit",
        AppMode::Normal => "?:help  q:quit",
        AppMode::CardDetail => "?:help  Esc:close",
        AppMode::Insert => insert_hint.as_deref().unwrap_or("Enter:confirm  Esc:cancel"),
        AppMode::Command => "Enter:search  Esc:cancel",
        AppMode::Dialog => "y:confirm  n:cancel",
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
