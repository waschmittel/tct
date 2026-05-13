use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, AppMode, CardDetailTab, InsertTarget};

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
            Span::styled("Esc", Style::default().fg(Color::Cyan)),
            Span::raw(":cancel  "),
            Span::styled("Ctrl+B/I/K", Style::default().fg(Color::Cyan)),
            Span::raw(":format  "),
            Span::styled("Ctrl+L", Style::default().fg(Color::Cyan)),
            Span::raw(":list  "),
            Span::styled("Ctrl+T", Style::default().fg(Color::Cyan)),
            Span::raw(":table "),
        ]
    } else {
        vec![
            Span::styled(" Esc", Style::default().fg(Color::Cyan)),
            Span::raw(":close  "),
            Span::styled("Tab", Style::default().fg(Color::Cyan)),
            Span::raw(":section  "),
            Span::styled("t", Style::default().fg(Color::Cyan)),
            Span::raw(":title "),
        ]
    };

    let block = Block::default()
        .title(title_display)
        .title_bottom(Line::from(bottom_hints))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    if inner.height < 3 {
        return;
    }

    // Tab bar
    let tab_area = Rect::new(inner.x, inner.y, inner.width, 1);
    let content_area = Rect::new(
        inner.x,
        inner.y + 2,
        inner.width,
        inner.height.saturating_sub(2),
    );

    let tabs = [
        CardDetailTab::Description,
        CardDetailTab::Checklists,
        CardDetailTab::Labels,
        CardDetailTab::DueDate,
    ];
    let tab_spans: Vec<Span> = tabs
        .iter()
        .map(|t| {
            if *t == board.detail_tab {
                Span::styled(
                    format!(" {} ", t.label()),
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled(format!(" {} ", t.label()), Style::default().fg(Color::Gray))
            }
        })
        .collect();
    frame.render_widget(Paragraph::new(Line::from(tab_spans)), tab_area);

    // Separator
    let sep_area = Rect::new(inner.x, inner.y + 1, inner.width, 1);
    frame.render_widget(
        Paragraph::new(Line::from("─".repeat(inner.width as usize)))
            .style(Style::default().fg(Color::DarkGray)),
        sep_area,
    );

    match board.detail_tab {
        CardDetailTab::Description => render_description(frame, content_area, card, app),
        CardDetailTab::Checklists => render_checklists(frame, content_area, card, board, app),
        CardDetailTab::Labels => render_labels(frame, content_area, card),
        CardDetailTab::DueDate => render_due_date(frame, content_area, card, app),
    }

    // Popup dialog for title editing
    if matches!(app.mode, AppMode::Insert(InsertTarget::EditCardTitle)) {
        render_input_dialog(frame, popup, "Edit Card Title", &app.input_buffer, app.input_cursor);
    }
}

fn render_description(
    frame: &mut Frame,
    area: Rect,
    card: &crate::model::card::Card,
    app: &App,
) {
    if let Some(textarea) = &app.description_editor {
        frame.render_widget(textarea, area);
        return;
    }

    if card.description.is_empty() {
        let text = Paragraph::new(Span::styled(
            "(no description — press 'e' to add)",
            Style::default().fg(Color::DarkGray),
        ));
        frame.render_widget(text, area);
    } else {
        // Render markdown
        let lines = markdown::render_markdown(&card.description);
        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);
    }
}

fn render_checklists(
    frame: &mut Frame,
    area: Rect,
    card: &crate::model::card::Card,
    board: &crate::app::LoadedBoard,
    app: &App,
) {
    if card.checklists.is_empty() {
        let text = Paragraph::new(Span::styled(
            "(no checklists — press 'A' to add)",
            Style::default().fg(Color::DarkGray),
        ));
        frame.render_widget(text, area);

        if let AppMode::Insert(InsertTarget::NewChecklistTitle) = &app.mode {
            render_input_dialog(frame, area, "New Checklist", &app.input_buffer, app.input_cursor);
        }
        return;
    }

    let mut lines = vec![];
    for (ci, cl) in card.checklists.iter().enumerate() {
        let is_active_cl = ci == board.detail_checklist_idx;
        let done = cl.items.iter().filter(|i| i.completed).count();
        let total = cl.items.len();

        let cl_style = if is_active_cl {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        lines.push(Line::from(Span::styled(
            format!("▸ {} [{}/{}]", cl.title, done, total),
            cl_style,
        )));

        for (ii, item) in cl.items.iter().enumerate() {
            let is_active = is_active_cl && ii == board.detail_item_idx;
            let check = if item.completed { "✓" } else { " " };
            let style = if is_active {
                Style::default()
                    .fg(Color::Cyan)
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
        lines.push(Line::raw(""));
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);

    match &app.mode {
        AppMode::Insert(InsertTarget::NewChecklistTitle) => {
            render_input_dialog(frame, area, "New Checklist", &app.input_buffer, app.input_cursor);
        }
        AppMode::Insert(InsertTarget::NewChecklistItem) => {
            render_input_dialog(frame, area, "New Item", &app.input_buffer, app.input_cursor);
        }
        AppMode::Insert(InsertTarget::EditChecklistItem) => {
            render_input_dialog(frame, area, "Edit Item", &app.input_buffer, app.input_cursor);
        }
        _ => {}
    }
}

fn render_labels(frame: &mut Frame, area: Rect, card: &crate::model::card::Card) {
    if card.labels.is_empty() {
        let text = Paragraph::new(Span::styled(
            "(no labels — press 'l' to add)",
            Style::default().fg(Color::DarkGray),
        ));
        frame.render_widget(text, area);
        return;
    }

    let mut lines = vec![];
    for label in &card.labels {
        lines.push(Line::from(Span::styled(
            format!("  ● {}", label.name),
            Style::default()
                .fg(Color::Black)
                .bg(label.color.to_ratatui_color()),
        )));
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

fn render_due_date(
    frame: &mut Frame,
    area: Rect,
    card: &crate::model::card::Card,
    app: &App,
) {
    let text = if let Some(due) = card.due_date {
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
        vec![
            Line::from(Span::styled(
                format!("  Date: {}", due.format("%Y-%m-%d")),
                Style::default().fg(Color::White),
            )),
            Line::from(Span::styled(format!("  {status}"), Style::default().fg(color))),
            Line::raw(""),
            Line::from(Span::styled(
                "  Press 'u' to change, or 'e' to edit",
                Style::default().fg(Color::DarkGray),
            )),
        ]
    } else {
        vec![
            Line::from(Span::styled(
                "  No due date set",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                "  Press 'u' to set (YYYY-MM-DD)",
                Style::default().fg(Color::DarkGray),
            )),
        ]
    };

    let paragraph = Paragraph::new(text);
    frame.render_widget(paragraph, area);

    if let AppMode::Insert(InsertTarget::EditDueDate) = &app.mode {
        render_input_dialog(
            frame,
            area,
            "Due Date (YYYY-MM-DD)",
            &app.input_buffer,
            app.input_cursor,
        );
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

    frame.render_widget(Paragraph::new(input), chunks[0]);
    frame.render_widget(
        Paragraph::new(Span::styled(
            "Enter: confirm  Esc: cancel",
            Style::default().fg(Color::DarkGray),
        )),
        chunks[1],
    );

    let cx = inner.x + cursor as u16;
    if cx < inner.x + inner.width {
        frame.set_cursor_position((cx, inner.y));
    }
}
