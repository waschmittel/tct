use chrono::{Datelike, NaiveDate};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    buffer: &str,
    cursor: usize,
    picker_date: Option<NaiveDate>,
    accent: Color,
) {
    let width = 25u16.min(area.width.saturating_sub(2));
    let height = 12u16.min(area.height.saturating_sub(2));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let dialog = Rect::new(x, y, width, height);

    frame.render_widget(Clear, dialog);

    let block = Block::default()
        .title(Line::from(Span::styled(
            " Due Date (YYYY-MM-DD) ",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent));
    let inner = block.inner(dialog);
    frame.render_widget(block, dialog);

    if inner.width == 0 || inner.height < 4 {
        return;
    }

    // Center the 21-column calendar block (7 × " dd") inside the popup.
    let content_w = 21u16.min(inner.width);
    let content = Rect::new(
        inner.x + (inner.width - content_w) / 2,
        inner.y,
        content_w,
        inner.height,
    );

    let chunks = Layout::vertical([
        Constraint::Length(1), // text input
        Constraint::Length(1), // blank
        Constraint::Length(1), // month header
        Constraint::Length(1), // day-of-week header
        Constraint::Min(6),    // calendar grid
    ])
    .split(content);

    // Text input line, indented one column to line up with the grid cells.
    let input_area = Rect::new(
        chunks[0].x + 1,
        chunks[0].y,
        chunks[0].width.saturating_sub(1),
        1,
    );
    let visible_w = input_area.width as usize;
    let cursor_char_idx = buffer[..cursor.min(buffer.len())].chars().count();
    let scroll = cursor_char_idx.saturating_sub(visible_w.saturating_sub(1));
    let visible: String = buffer.chars().skip(scroll).take(visible_w).collect();
    frame.render_widget(Paragraph::new(visible), input_area);
    let cx = input_area.x + (cursor_char_idx - scroll) as u16;
    if cx < input_area.x + input_area.width {
        frame.set_cursor_position((cx, input_area.y));
    }

    // Calendar grid (only if we have a parsed date)
    let date = picker_date.unwrap_or_else(|| chrono::Local::now().date_naive());
    let month_title = format!("{} {}", month_name(date.month()), date.year());
    let title_pad = (chunks[2].width as usize).saturating_sub(month_title.chars().count()) / 2;
    let month_line = format!("{:pad$}{}", "", month_title, pad = title_pad);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            month_line,
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ))),
        chunks[2],
    );

    let dow_header = " Mo Tu We Th Fr Sa Su";
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            dow_header,
            Style::default().fg(Color::DarkGray),
        ))),
        chunks[3],
    );

    let grid_lines = build_grid(date, accent);
    frame.render_widget(Paragraph::new(grid_lines), chunks[4]);
}

fn month_name(month: u32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "",
    }
}

fn build_grid(selected: NaiveDate, accent: Color) -> Vec<Line<'static>> {
    let first_of_month = NaiveDate::from_ymd_opt(selected.year(), selected.month(), 1).unwrap();
    // Monday = 0 ... Sunday = 6
    let leading = first_of_month.weekday().num_days_from_monday() as usize;
    let days = days_in_month(selected.year(), selected.month());

    let today = chrono::Local::now().date_naive();

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut col = 0usize;

    // Leading blanks
    for _ in 0..leading {
        spans.push(Span::raw("   "));
        col += 1;
        if col == 7 {
            lines.push(Line::from(std::mem::take(&mut spans)));
            col = 0;
        }
    }

    for day in 1..=days {
        let is_selected = day == selected.day();
        let date = NaiveDate::from_ymd_opt(selected.year(), selected.month(), day).unwrap();
        let is_today = date == today;

        let style = if is_selected {
            Style::default()
                .fg(Color::Black)
                .bg(accent)
                .add_modifier(Modifier::BOLD)
        } else if is_today {
            Style::default().fg(accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        spans.push(Span::raw(" "));
        spans.push(Span::styled(format!("{day:>2}"), style));

        col += 1;
        if col == 7 {
            lines.push(Line::from(std::mem::take(&mut spans)));
            col = 0;
        }
    }

    if !spans.is_empty() {
        lines.push(Line::from(spans));
    }

    lines
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let (ny, nm) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let first_next = NaiveDate::from_ymd_opt(ny, nm, 1).unwrap();
    first_next.pred_opt().unwrap().day()
}

