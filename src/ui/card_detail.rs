use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, AppMode, InsertTarget};

use super::markdown;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let board = match &app.board {
        Some(b) => b,
        None => return,
    };
    let card = match board.current_card() {
        Some(c) => c,
        None => return,
    };

    let accent = app.accent_color();

    let width = (area.width * 80 / 100).max(40).min(area.width.saturating_sub(2));
    let height = (area.height * 80 / 100).max(20).min(area.height.saturating_sub(2));
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup);

    let title_display = format!(" {} ", card.title);

    let is_editing_desc = matches!(app.mode, AppMode::Insert(InsertTarget::EditCardDescription));

    let bottom_hints = if is_editing_desc {
        vec![
            Span::styled(" Ctrl+S", Style::default().fg(Color::Yellow)),
            Span::raw(":save  "),
            Span::styled("Esc", Style::default().fg(accent)),
            Span::raw(":cancel  "),
            Span::styled("Ctrl+B/I/K", Style::default().fg(accent)),
            Span::raw(":format  "),
            Span::styled("Ctrl+L", Style::default().fg(accent)),
            Span::raw(":list "),
        ]
    } else {
        vec![
            Span::styled(" Esc", Style::default().fg(accent)),
            Span::raw(":close  "),
            Span::styled("t", Style::default().fg(accent)),
            Span::raw(":title  "),
            Span::styled("e", Style::default().fg(accent)),
            Span::raw(":desc  "),
            Span::styled("u", Style::default().fg(accent)),
            Span::raw(":due  "),
            Span::styled("l", Style::default().fg(accent)),
            Span::raw(":labels  "),
            Span::styled("a", Style::default().fg(accent)),
            Span::raw(":add  "),
            Span::styled("Space", Style::default().fg(accent)),
            Span::raw(":toggle "),
        ]
    };

    let block = Block::default()
        .title(title_display)
        .title_bottom(Line::from(bottom_hints))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    if inner.height < 3 {
        return;
    }

    // If editing description, render the editor instead
    if is_editing_desc {
        if let Some(textarea) = &app.description_editor {
            render_description_editor(frame, inner, textarea, app.editor_scroll, accent);
        }
        return;
    }

    // Unified view: render all sections vertically
    let mut lines: Vec<Line<'static>> = Vec::new();

    // --- Description Section ---
    lines.push(Line::from(Span::styled(
        "Description",
        Style::default().fg(accent).add_modifier(Modifier::BOLD),
    )));
    if card.description.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (no description — press 'e' to add)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        let desc_lines = markdown::highlight_lines(&card.description, accent);
        for dl in desc_lines {
            lines.push(dl);
        }
    }

    lines.push(Line::from(Span::styled(
        "─".repeat(inner.width as usize),
        Style::default().fg(Color::DarkGray),
    )));

    // --- Checklist Section ---
    let (done, total) = card.checklist_progress().unwrap_or((0, 0));
    if total > 0 {
        lines.push(Line::from(Span::styled(
            format!("Checklist [{done}/{total}]"),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "Checklist",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        )));
    }

    if card.checklist.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (no items — press 'a' to add)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (ii, item) in card.checklist.iter().enumerate() {
            let is_active = ii == board.detail_item_idx;
            let check = if item.completed { "✓" } else { " " };
            let style = if is_active {
                Style::default()
                    .fg(accent)
                    .add_modifier(Modifier::BOLD)
            } else if item.completed {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };
            let prefix = if is_active { "» " } else { "  " };
            lines.push(Line::from(Span::styled(
                format!("{prefix}[{check}] {}", item.text),
                style,
            )));
        }
    }

    lines.push(Line::from(Span::styled(
        "─".repeat(inner.width as usize),
        Style::default().fg(Color::DarkGray),
    )));

    // --- Labels Section ---
    lines.push(Line::from(Span::styled(
        "Labels",
        Style::default().fg(accent).add_modifier(Modifier::BOLD),
    )));

    let resolved = card.resolved_labels(&board.meta.labels);
    if resolved.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (no labels — press 'l' to add)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for label in &resolved {
            lines.push(Line::from(Span::styled(
                format!("  ● {}", label.name),
                Style::default()
                    .fg(Color::Black)
                    .bg(label.color.to_ratatui_color()),
            )));
        }
    }

    lines.push(Line::from(Span::styled(
        "─".repeat(inner.width as usize),
        Style::default().fg(Color::DarkGray),
    )));

    // --- Due Date Section ---
    lines.push(Line::from(Span::styled(
        "Due Date",
        Style::default().fg(accent).add_modifier(Modifier::BOLD),
    )));

    if let Some(due) = card.due_date {
        let today = chrono::Local::now().date_naive();
        let days = (due - today).num_days();
        let (status, color) = if days < 0 {
            (format!("{} days overdue", -days), Color::Red)
        } else if days == 0 {
            ("Due today!".to_string(), Color::Yellow)
        } else if days <= 3 {
            (format!("Due in {} days", days), Color::Yellow)
        } else {
            (format!("Due in {} days", days), Color::Green)
        };
        lines.push(Line::from(Span::styled(
            format!("  {}", due.format("%Y-%m-%d")),
            Style::default().fg(Color::White),
        )));
        lines.push(Line::from(Span::styled(
            format!("  {status}"),
            Style::default().fg(color),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "  (no due date — press 'u' to set)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);

    // Input dialogs rendered on top
    match &app.mode {
        AppMode::Insert(InsertTarget::EditCardTitle) => {
            render_input_dialog(frame, popup, "Edit Card Title", &app.input_buffer, app.input_cursor);
        }
        AppMode::Insert(InsertTarget::NewChecklistItem) => {
            render_input_dialog(frame, popup, "New Item", &app.input_buffer, app.input_cursor);
        }
        AppMode::Insert(InsertTarget::EditChecklistItem) => {
            render_input_dialog(frame, popup, "Edit Item", &app.input_buffer, app.input_cursor);
        }
        AppMode::Insert(InsertTarget::EditDueDate) => {
            render_input_dialog(
                frame,
                popup,
                "Due Date (YYYY-MM-DD)",
                &app.input_buffer,
                app.input_cursor,
            );
        }
        _ => {}
    }
}

fn render_description_editor(
    frame: &mut Frame,
    area: Rect,
    textarea: &ratatui_textarea::TextArea<'static>,
    editor_scroll: usize,
    accent: Color,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" Edit Description ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let visible_height = inner.height as usize;
    let lines = textarea.lines();
    let ratatui_textarea::DataCursor(cursor_row, cursor_col) = textarea.cursor();

    let scroll = editor_scroll;
    let end = (scroll + visible_height).min(lines.len());
    let start = scroll.min(end);

    for (vi, li) in (start..end).enumerate() {
        let line_text = &lines[li];
        let highlighted = markdown::highlight_line(line_text, accent);

        let y = inner.y + vi as u16;
        let line_area = Rect::new(inner.x, y, inner.width, 1);

        if li == cursor_row {
            frame.render_widget(
                Paragraph::new(Line::from(highlighted))
                    .style(Style::default().bg(Color::Rgb(30, 30, 40))),
                line_area,
            );
        } else {
            frame.render_widget(Paragraph::new(Line::from(highlighted)), line_area);
        }
    }

    if cursor_row >= start && cursor_row < end {
        let cx = inner.x + (cursor_col as u16).min(inner.width.saturating_sub(1));
        let cy = inner.y + (cursor_row - start) as u16;
        frame.set_cursor_position((cx, cy));
    }
}

fn render_input_dialog(frame: &mut Frame, area: Rect, title: &str, input: &str, cursor: usize) {
    let width = 50u16.min(area.width.saturating_sub(2));
    let height = 5u16;
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let dialog = Rect::new(x, y, width, height);

    frame.render_widget(Clear, dialog);

    let block = Block::default()
        .title(format!(" {title} "))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let inner = block.inner(dialog);
    frame.render_widget(block, dialog);

    let chunks = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(inner);

    let visible_w = chunks[0].width as usize;
    let scroll = if cursor >= visible_w {
        cursor - visible_w + 1
    } else {
        0
    };
    let end = (scroll + visible_w).min(input.len());
    let visible = &input[scroll..end];
    frame.render_widget(Paragraph::new(visible), chunks[0]);
    frame.render_widget(
        Paragraph::new(Span::styled(
            "Enter: confirm  Esc: cancel",
            Style::default().fg(Color::DarkGray),
        )),
        chunks[1],
    );

    let cx = inner.x + (cursor - scroll) as u16;
    if cx < inner.x + inner.width {
        frame.set_cursor_position((cx, inner.y));
    }
}
