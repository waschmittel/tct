use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{App, DialogKind};

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
        DialogKind::ConfirmArchiveBoard => {
            let name = app
                .boards
                .get(app.selected_board_idx)
                .map(|b| b.name.as_str())
                .unwrap_or("this board");
            render_confirm(
                frame,
                area,
                "Archive Board",
                &format!("Archive board '{name}'?"),
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
        DialogKind::ArchivedBoards => {
            render_archived_boards(frame, area, app);
        }
        DialogKind::LabelPicker => {
            render_label_picker(frame, area, app);
        }
        DialogKind::LabelManager => {
            render_label_manager(frame, area, app);
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

fn accent(app: &App) -> Color {
    app.accent_color()
}

fn render_archived_cards(frame: &mut Frame, area: Rect, app: &App) {
    let height = (app.archived_cards.len() as u16 + 4).min(area.height.saturating_sub(4)).max(6);
    let width = 50u16.min(area.width.saturating_sub(4));
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Archived Cards ")
        .title_bottom(Line::from(vec![
            Span::styled(" Enter", Style::default().fg(Color::Green)),
            Span::raw(":restore  "),
            Span::styled("x", Style::default().fg(Color::Red)),
            Span::raw(":delete  "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(":close "),
        ]))
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
            Style::default().fg(accent(app)).add_modifier(Modifier::BOLD)
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
    let board = match &app.board {
        Some(b) => b,
        None => return,
    };

    let board_labels = &board.meta.labels;
    let card_label_ids: Vec<_> = board
        .current_card()
        .map(|c| c.label_ids.as_slice())
        .unwrap_or(&[])
        .to_vec();

    let height = (board_labels.len() as u16 + 6).min(area.height.saturating_sub(4)).max(6);
    let width = 36u16.min(area.width.saturating_sub(4));
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Labels (Space/Enter:toggle, Esc:back) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent(app)));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    if board_labels.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "No labels. Press L to manage.",
                Style::default().fg(Color::DarkGray),
            )),
            inner,
        );
        return;
    }

    let mut lines = vec![];
    for (i, label) in board_labels.iter().enumerate() {
        let assigned = card_label_ids.contains(&label.id);
        let is_selected = i == app.label_picker_idx;
        let check = if assigned { "●" } else { "○" };
        let label_style = Style::default().fg(label.color.to_ratatui_color());

        if is_selected {
            lines.push(Line::from(vec![
                Span::styled(
                    "» ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{check} {}", label.name),
                    label_style.add_modifier(Modifier::BOLD),
                ),
            ]));
        } else {
            lines.push(Line::from(Span::styled(
                format!("  {check} {}", label.name),
                label_style,
            )));
        }
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_label_manager(frame: &mut Frame, area: Rect, app: &App) {
    let board = match &app.board {
        Some(b) => b,
        None => return,
    };

    let labels = &board.meta.labels;
    let height = (labels.len() as u16 + 6).min(area.height.saturating_sub(4)).max(8);
    let width = 40u16.min(area.width.saturating_sub(4));
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Label Manager ")
        .title_bottom(Line::from(vec![
            Span::styled(" n", Style::default().fg(accent(app))),
            Span::raw(":new  "),
            Span::styled("e", Style::default().fg(accent(app))),
            Span::raw(":rename  "),
            Span::styled("c", Style::default().fg(accent(app))),
            Span::raw(":color  "),
            Span::styled("x", Style::default().fg(accent(app))),
            Span::raw(":delete  "),
            Span::styled("Esc", Style::default().fg(accent(app))),
            Span::raw(":close "),
        ]))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    if labels.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "No labels. Press 'n' to create one.",
                Style::default().fg(Color::DarkGray),
            )),
            inner,
        );
        return;
    }

    let mut lines = vec![];
    for (i, label) in labels.iter().enumerate() {
        let is_selected = i == app.label_picker_idx;
        let label_style = Style::default()
            .fg(Color::Black)
            .bg(label.color.to_ratatui_color());

        if is_selected {
            lines.push(Line::from(vec![
                Span::styled(
                    "» ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" {} ", label.name),
                    label_style.add_modifier(Modifier::BOLD),
                ),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(format!(" {} ", label.name), label_style),
            ]));
        }
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_archived_boards(frame: &mut Frame, area: Rect, app: &App) {
    let height = (app.archived_boards.len() as u16 + 5).min(area.height.saturating_sub(4)).max(6);
    let width = 54u16.min(area.width.saturating_sub(4));
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Archived Boards ")
        .title_bottom(Line::from(vec![
            Span::styled(" Enter", Style::default().fg(Color::Green)),
            Span::raw(":restore  "),
            Span::styled("x", Style::default().fg(Color::Red)),
            Span::raw(":delete  "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(":close "),
        ]))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    if app.archived_boards.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled("No archived boards", Style::default().fg(Color::DarkGray))),
            inner,
        );
        return;
    }

    let mut lines = Vec::new();
    for (i, board) in app.archived_boards.iter().enumerate() {
        let is_selected = i == app.archived_selected;
        let prefix = if is_selected { "» " } else { "  " };
        let board_color = board.accent_color.to_ratatui_color();
        let style = if is_selected {
            Style::default().fg(board_color).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(board_color)
        };
        lines.push(Line::from(Span::styled(format!("{prefix}{}", board.name), style)));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}
