use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{App, AppMode, InsertTarget};

use super::widgets::list_widget;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let board = match &app.board {
        Some(b) => b,
        None => return,
    };

    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(2),
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
        let empty = Paragraph::new(Line::from(vec![
            Span::styled("  No lists. Press ", Style::default().fg(Color::DarkGray)),
            Span::styled("N", Style::default().fg(accent)),
            Span::styled(" to create one.", Style::default().fg(Color::DarkGray)),
        ]));
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
    match &app.mode {
        AppMode::Insert(InsertTarget::NewCardTitle) => {
            render_input_overlay(frame, area, "New Card", &app.input_buffer, app.input_cursor, accent);
        }
        AppMode::Insert(InsertTarget::NewListName) => {
            render_input_overlay(frame, area, "New List", &app.input_buffer, app.input_cursor, accent);
        }
        AppMode::Insert(InsertTarget::EditCardTitleInline) => {
            render_input_overlay(
                frame,
                area,
                "Edit Card Title",
                &app.input_buffer,
                app.input_cursor,
                accent,
            );
        }
        AppMode::Insert(InsertTarget::RenameList) => {
            render_input_overlay(
                frame,
                area,
                "Rename List",
                &app.input_buffer,
                app.input_cursor,
                accent,
            );
        }
        AppMode::Insert(InsertTarget::NewLabelName) => {
            render_input_overlay(frame, area, "New Label", &app.input_buffer, app.input_cursor, accent);
        }
        AppMode::Insert(InsertTarget::EditLabelName) => {
            render_input_overlay(
                frame,
                area,
                "Rename Label",
                &app.input_buffer,
                app.input_cursor,
                accent,
            );
        }
        AppMode::Insert(InsertTarget::EditDueDate) => {
            render_input_overlay(
                frame,
                area,
                "Due Date (YYYY-MM-DD)",
                &app.input_buffer,
                app.input_cursor,
                accent,
            );
        }
        _ => {}
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
    let height = 5u16;
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

    let chunks = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(inner);

    let visible_w = chunks[0].width as usize;
    let cursor_char_idx = input[..cursor].chars().count();
    let scroll = if cursor_char_idx >= visible_w {
        cursor_char_idx - visible_w + 1
    } else {
        0
    };
    
    let visible: String = input.chars().skip(scroll).take(visible_w).collect();
    let text = Paragraph::new(Line::from(Span::raw(visible)));
    frame.render_widget(text, chunks[0]);

    let hints = Paragraph::new(Line::from(Span::styled(
        "Enter: confirm  Esc: cancel",
        Style::default().fg(Color::DarkGray),
    )));
    frame.render_widget(hints, chunks[1]);

    let cx = inner.x + (cursor_char_idx - scroll) as u16;
    let cy = inner.y;
    if cx < inner.x + inner.width {
        frame.set_cursor_position((cx, cy));
    }
}
