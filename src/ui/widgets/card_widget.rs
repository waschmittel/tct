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
    board_labels: &[Label],
    accent: Color,
) {
    if area.height < 2 {
        return;
    }

    let base_style = if dimmed {
        Style::default().fg(Color::DarkGray)
    } else if selected {
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let border_style = if selected {
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
    if !resolved.is_empty() && (lines.len() as u16) < inner.height {
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

    if (lines.len() as u16) < inner.height {
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
        if card.has_description() {
            let style = if dimmed {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Gray)
            };
            if !info.is_empty() {
                info.push(Span::raw(" "));
            }
            info.push(Span::styled("≡", style));
        }
        if !info.is_empty() {
            lines.push(Line::from(info));
        }
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::card::ChecklistItem;

    fn build_lines(card: &Card, board_labels: &[Label], inner_height: u16) -> Vec<Line<'static>> {
        let base_style = Style::default();
        let mut lines = vec![];
        lines.push(Line::from(Span::styled(
            card.title.clone(),
            base_style.add_modifier(Modifier::BOLD),
        )));

        let resolved = card.resolved_labels(board_labels);
        if !resolved.is_empty() && (lines.len() as u16) < inner_height {
            let label_spans: Vec<Span> = resolved
                .iter()
                .map(|l| {
                    Span::styled(
                        format!("[{}]", l.name),
                        Style::default()
                            .fg(Color::Black)
                            .bg(l.color.to_ratatui_color()),
                    )
                })
                .collect();
            lines.push(Line::from(label_spans));
        }

        if (lines.len() as u16) < inner_height {
            let mut info = vec![];
            if card.due_date.is_some() {
                info.push(Span::raw("due"));
            }
            if card.checklist_progress().is_some() {
                info.push(Span::raw("checklist"));
            }
            if card.has_description() {
                info.push(Span::styled("≡", Style::default()));
            }
            if !info.is_empty() {
                lines.push(Line::from(info));
            }
        }
        lines
    }

    #[test]
    fn info_line_shown_without_labels() {
        let mut card = Card::new("Test".into());
        card.description = "has desc".into();
        card.checklist = vec![ChecklistItem {
            text: "item".into(),
            completed: false,
        }];
        // inner_height = 2 (title + info, no labels)
        let lines = build_lines(&card, &[], 2);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn info_line_shown_with_labels() {
        let mut card = Card::new("Test".into());
        card.description = "has desc".into();
        let label = Label {
            id: "l1".into(),
            name: "bug".into(),
            color: crate::model::label::LabelColor::Red,
        };
        card.label_ids = vec!["l1".into()];
        // inner_height = 3 (title + labels + info)
        let lines = build_lines(&card, &[label], 3);
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn no_info_line_when_no_metadata() {
        let card = Card::new("Test".into());
        let lines = build_lines(&card, &[], 2);
        assert_eq!(lines.len(), 1);
    }
}
