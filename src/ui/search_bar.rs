use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let bar_area = Rect::new(area.x, area.height.saturating_sub(3), area.width, 1);

    frame.render_widget(Clear, bar_area);

    let line = Line::from(vec![
        Span::styled(" /", Style::default().fg(app.accent_color())),
        Span::raw(&app.search_query),
    ]);
    frame.render_widget(Paragraph::new(line), bar_area);

    let cx = bar_area.x + 2 + app.search_query.chars().count() as u16;
    if cx < bar_area.x + bar_area.width {
        frame.set_cursor_position((cx, bar_area.y));
    }
}
