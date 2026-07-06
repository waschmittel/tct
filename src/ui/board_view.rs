use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{App, AppMode};
use crate::insert::InsertSurface;

use super::widgets::list_widget;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let board = match app.board() {
        Some(b) => b,
        None => return,
    };

    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(area);

    let accent = app.accent_color();

    // Title bar
    let mut title_spans = vec![
        Span::styled(
            format!(" {} ", board.meta.name),
            Style::default()
                .fg(accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("({} lists)", board.lists.len()),
            Style::default().fg(Color::DarkGray),
        ),
    ];

    if app.search_active {
        title_spans.push(Span::raw("  "));
        title_spans.push(Span::styled(
            format!("[search: {}]", app.search_query),
            Style::default().fg(Color::Yellow),
        ));
    }
    if app.label_filter.is_some() {
        title_spans.push(Span::raw("  "));
        title_spans.push(Span::styled(
            "[label filter active]",
            Style::default().fg(Color::Yellow),
        ));
    }

    frame.render_widget(Paragraph::new(Line::from(title_spans)), chunks[0]);

    // Lists area
    if board.lists.is_empty() {
        let empty = Paragraph::new(Line::from(Span::styled(
            "  No lists.",
            Style::default().fg(Color::DarkGray),
        )));
        frame.render_widget(empty, chunks[1]);
    } else {
        let num_lists = board.lists.len();
        let constraints: Vec<Constraint> = (0..num_lists)
            .map(|_| Constraint::Percentage((100 / num_lists as u16).max(1)))
            .collect();
        let list_areas = Layout::horizontal(constraints).split(chunks[1]);

        for (i, list) in board.lists.iter().enumerate() {
            let is_selected = i == board.selected_list;
            list_widget::render(frame, list_areas[i], list, i, is_selected, app);
        }
    }

    // Inline title editing overlay (for new card / new list / rename)
    if matches!(&app.mode, AppMode::Insert)
        && let Some(handler) = app.insert.as_ref()
        && handler.surface() == InsertSurface::BoardView
    {
        // Date picker for editing due date from list view is one of the
        // BoardView handlers; everything else is a line editor.
        if let Some(dp) = handler
            .as_any()
            .downcast_ref::<crate::insert::date_picker::DatePicker>()
        {
            super::widgets::date_picker::render(
                frame,
                area,
                &dp.buffer,
                dp.cursor,
                dp.picker_date,
                accent,
            );
        } else if let (Some(buf), Some(cursor)) =
            (handler.line_buffer(), handler.line_cursor())
        {
            render_input_overlay(frame, area, handler.title(), buf, cursor, accent);
        }
    }

    // Status bar
    super::status_bar::render(frame, chunks[2], app);
}

fn render_input_overlay(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    input: &str,
    cursor: usize,
    accent: Color,
) {
    let width = 50u16.min(area.width.saturating_sub(4));
    let height = 3u16;
    let x = (area.width.saturating_sub(width)) / 2 + area.x;
    let y = (area.height.saturating_sub(height)) / 2 + area.y;
    let dialog_area = Rect::new(x, y, width, height);

    frame.render_widget(ratatui::widgets::Clear, dialog_area);

    let block = ratatui::widgets::Block::default()
        .title(format!(" {title} "))
        .borders(ratatui::widgets::Borders::ALL)
        .border_style(Style::default().fg(accent));
    let inner = block.inner(dialog_area);
    frame.render_widget(block, dialog_area);

    let visible_w = inner.width as usize;
    let cursor_char_idx = input[..cursor].chars().count();
    let scroll = if cursor_char_idx >= visible_w {
        cursor_char_idx - visible_w + 1
    } else {
        0
    };

    let visible: String = input.chars().skip(scroll).take(visible_w).collect();
    let text = Paragraph::new(Line::from(Span::raw(visible)));
    frame.render_widget(text, inner);

    let cx = inner.x + (cursor_char_idx - scroll) as u16;
    let cy = inner.y;
    if cx < inner.x + inner.width {
        frame.set_cursor_position((cx, cy));
    }
}
