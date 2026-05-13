use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::model::card::Card;
use crate::model::label::Label;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    card: &Card,
    selected: bool,
    dimmed: bool,
    grabbed: bool,
    board_labels: &[Label],
    accent: Color,
) {
    if area.height < 2 {
        return;
    }

    let base_style = if dimmed {
        Style::default().fg(Color::DarkGray)
    } else if grabbed {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else if selected {
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let border_style = if grabbed {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else if selected {
        Style::default().fg(accent).add_modifier(Modifier::BOLD)
    } else if dimmed {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::Gray)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let mut lines = vec![];

    lines.push(Line::from(Span::styled(card.title.clone(), base_style.add_modifier(Modifier::BOLD))));

    let resolved = card.resolved_labels(board_labels);
    if !resolved.is_empty() && inner.height > 1 {
        let label_spans: Vec<Span> = resolved
            .iter()
            .map(|l| {
                if dimmed {
                    Span::styled(format!("[{}]", l.name), Style::default().fg(Color::DarkGray))
                } else {
                    Span::styled(
                        format!("[{}]", l.name),
                        Style::default()
                            .fg(Color::Black)
                            .bg(l.color.to_ratatui_color()),
                    )
                }
            })
            .collect();
        lines.push(Line::from(label_spans));
    }

    if inner.height > 2 {
        let mut info = vec![];
        if let Some(due) = &card.due_date {
            let today = chrono::Local::now().date_naive();
            let days = (*due - today).num_days();
            let color = if dimmed {
                Color::DarkGray
            } else if days < 0 {
                Color::Red
            } else if days <= 3 {
                Color::Yellow
            } else {
                Color::Green
            };
            let label = if days < 0 {
                format!("Due:{} (-{}d)", due.format("%m/%d"), -days)
            } else if days == 0 {
                format!("Due:{} (today)", due.format("%m/%d"))
            } else if days <= 3 {
                format!("Due:{} ({}d)", due.format("%m/%d"), days)
            } else {
                format!("Due:{}", due.format("%m/%d"))
            };
            info.push(Span::styled(label, Style::default().fg(color)));
        }
        if let Some((done, total)) = card.checklist_progress() {
            let style = if dimmed {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Gray)
            };
            if !info.is_empty() {
                info.push(Span::raw(" "));
            }
            info.push(Span::styled(format!("[{done}/{total}]"), style));
        }
        if !info.is_empty() {
            lines.push(Line::from(info));
        }
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}
