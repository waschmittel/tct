use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{App, DialogKind};
use crate::model::label::LabelColor;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let kind = match &app.mode {
        crate::app::AppMode::Dialog(k) => k,
        _ => return,
    };

    match kind {
        DialogKind::ConfirmDeleteCard => {
            let name = app
                .board
                .as_ref()
                .and_then(|b| b.current_card())
                .map(|c| c.title.as_str())
                .unwrap_or("this card");
            render_confirm(frame, area, "Delete Card", &format!("Delete '{name}'?"));
        }
        DialogKind::ConfirmDeleteList => {
            let name = app
                .board
                .as_ref()
                .and_then(|b| b.lists.get(b.selected_list))
                .map(|l| l.name.as_str())
                .unwrap_or("this list");
            render_confirm(
                frame,
                area,
                "Delete List",
                &format!("Delete list '{name}' and all its cards?"),
            );
        }
        DialogKind::ConfirmDeleteBoard => {
            let name = app
                .boards
                .get(app.selected_board_idx)
                .map(|b| b.name.as_str())
                .unwrap_or("this board");
            render_confirm(
                frame,
                area,
                "Delete Board",
                &format!("Delete board '{name}' and all its data?"),
            );
        }
        DialogKind::ConfirmArchiveCard => {
            let name = app
                .board
                .as_ref()
                .and_then(|b| b.current_card())
                .map(|c| c.title.as_str())
                .unwrap_or("this card");
            render_confirm(frame, area, "Archive Card", &format!("Archive '{name}'?"));
        }
        DialogKind::ConfirmCancelEdit => {
            render_confirm(
                frame,
                area,
                "Discard Changes",
                "Discard unsaved changes?",
            );
        }
        DialogKind::ArchivedCards => {
            render_archived_cards(frame, area, app);
        }
        DialogKind::LabelPicker => {
            render_label_picker(frame, area, app);
        }
    }
}

fn render_confirm(frame: &mut Frame, area: Rect, title: &str, message: &str) {
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

fn render_archived_cards(frame: &mut Frame, area: Rect, app: &App) {
    let height = (app.archived_cards.len() as u16 + 4).min(area.height.saturating_sub(4)).max(6);
    let width = 50u16.min(area.width.saturating_sub(4));
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Archived Cards (Enter: restore, Esc: close) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    if app.archived_cards.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled("No archived cards", Style::default().fg(Color::DarkGray))),
            inner,
        );
        return;
    }

    let mut lines = Vec::new();
    for (i, card) in app.archived_cards.iter().enumerate() {
        let is_selected = i == app.archived_selected;
        let prefix = if is_selected { "» " } else { "  " };
        let date = card.updated_at.format("%Y-%m-%d");
        let style = if is_selected {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        lines.push(Line::from(Span::styled(
            format!("{prefix}{} ({})", card.title, date),
            style,
        )));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_label_picker(frame: &mut Frame, area: Rect, app: &App) {
    let all = LabelColor::all();
    let current_labels = app
        .board
        .as_ref()
        .and_then(|b| b.current_card())
        .map(|c| &c.labels[..])
        .unwrap_or(&[]);

    let height = (all.len() as u16 + 4).min(area.height.saturating_sub(4));
    let width = 30u16.min(area.width.saturating_sub(4));
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Labels (Enter to toggle) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let mut lines = vec![];
    for (i, color) in all.iter().enumerate() {
        let is_active = current_labels.iter().any(|l| l.color == *color);
        let is_selected = i == app.label_picker_idx;
        let check = if is_active { "●" } else { "○" };
        let prefix = if is_selected { "» " } else { "  " };

        let style = if is_selected {
            Style::default()
                .fg(color.to_ratatui_color())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(color.to_ratatui_color())
        };

        lines.push(Line::from(Span::styled(
            format!("{prefix}{check} {}", color.name()),
            style,
        )));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}
