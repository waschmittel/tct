use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::app::{App, AppMode, InsertTarget};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(2),
    ])
    .split(area);

    let title = Paragraph::new(Line::from(vec![
        Span::styled(" TCT ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw("- Terminal Card Tracker"),
    ]))
    .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(title, chunks[0]);

    if app.boards.is_empty() {
        let empty = Paragraph::new(Line::from(vec![
            Span::styled("  No boards yet. Press ", Style::default().fg(Color::DarkGray)),
            Span::styled("n", Style::default().fg(Color::Cyan)),
            Span::styled(" to create one.", Style::default().fg(Color::DarkGray)),
        ]));
        frame.render_widget(empty, chunks[1]);
    } else {
        let items: Vec<ListItem> = app
            .boards
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let board_color = b.accent_color.to_ratatui_color();
                let style = if i == app.selected_board_idx {
                    Style::default().fg(board_color).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(board_color)
                };
                let prefix = if i == app.selected_board_idx { ">" } else { " " };
                ListItem::new(Line::from(Span::styled(
                    format!("{prefix} {}", b.name),
                    style,
                )))
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)).title(" Boards "));
        let mut state = ListState::default().with_selected(Some(app.selected_board_idx));
        frame.render_stateful_widget(list, chunks[1], &mut state);
    }

    if let AppMode::Insert(InsertTarget::NewBoardName) = &app.mode {
        render_input_dialog(frame, area, "New Board", &app.input_buffer, app.input_cursor);
    }
    if let AppMode::Insert(InsertTarget::RenameBoard) = &app.mode {
        render_input_dialog(frame, area, "Rename Board", &app.input_buffer, app.input_cursor);
    }

    super::status_bar::render(frame, chunks[2], app);
}

fn render_input_dialog(frame: &mut Frame, area: Rect, title: &str, input: &str, cursor: usize) {
    let width = 40u16.min(area.width.saturating_sub(4));
    let height = 5u16;
    let x = (area.width.saturating_sub(width)) / 2 + area.x;
    let y = (area.height.saturating_sub(height)) / 2 + area.y;
    let dialog_area = Rect::new(x, y, width, height);

    frame.render_widget(ratatui::widgets::Clear, dialog_area);

    let block = Block::default()
        .title(format!(" {title} "))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
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
