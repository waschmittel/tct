use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use crate::model::card::Card;
use crate::model::label::Label;
use crate::term_caps::TermCaps;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    card: &Card,
    selected: bool,
    dimmed: bool,
    board_labels: &[Label],
    accent: Color,
    caps: TermCaps,
) {
    if area.height < 2 {
        return;
    }

    let selection_bg = caps.selection_bg();
    let base_style = if dimmed {
        Style::default().fg(Color::DarkGray)
    } else if selected {
        Style::default()
            .fg(Color::White)
            .bg(selection_bg)
            .add_modifier(Modifier::BOLD)
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

    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .border_type(if selected {
            caps.selected_border_type()
        } else {
            BorderType::Plain
        });
    if selected && !dimmed {
        block = block.style(Style::default().bg(selection_bg));
    }
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    // Lines are pre-wrapped by card_lines and the paragraph does no
    // wrapping of its own, so `list_widget::card_height` (which counts
    // the same lines) can never disagree with what gets rendered.
    let lines = card_lines(card, board_labels, inner.width, base_style, dimmed);
    frame.render_widget(Paragraph::new(lines), inner);
}

/// Builds a card's rendered lines, pre-wrapped at `width` columns:
/// wrapped title, wrapped label chips, then the info line (due date,
/// checklist progress, description marker). Single source of truth for
/// both rendering and `list_widget::card_height` — any drift between
/// the two leaves blank rows or clips the info line.
pub fn card_lines(
    card: &Card,
    board_labels: &[Label],
    width: u16,
    base_style: Style,
    dimmed: bool,
) -> Vec<Line<'static>> {
    let mut lines = crate::ui::markdown::wrap_spans_with_indent(
        vec![Span::styled(
            card.title.clone(),
            base_style.add_modifier(Modifier::BOLD),
        )],
        width as usize,
        0,
    );

    let resolved = card.resolved_labels(board_labels);
    lines.extend(super::labels::label_lines(&resolved, width as usize, dimmed));

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
            format!("{} (-{}d)", due.format("%m-%d"), -days)
        } else if days == 0 {
            format!("{} (today)", due.format("%m-%d"))
        } else if days <= 3 {
            format!("{} ({}d)", due.format("%m-%d"), days)
        } else {
            due.format("%m-%d").to_string()
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

    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::card::ChecklistItem;
    use crate::model::label::LabelColor;

    fn lines_at(card: &Card, board_labels: &[Label], width: u16) -> Vec<Line<'static>> {
        card_lines(card, board_labels, width, Style::default(), false)
    }

    #[test]
    fn info_line_shown_without_labels() {
        let mut card = Card::new("Test".into());
        card.description = "has desc".into();
        card.checklist = vec![ChecklistItem {
            text: "item".into(),
            completed: false,
        }];
        // title + info
        let lines = lines_at(&card, &[], 40);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn info_line_shown_with_labels() {
        let mut card = Card::new("Test".into());
        card.description = "has desc".into();
        let label = Label {
            id: "l1".into(),
            name: "bug".into(),
            color: LabelColor::Red,
        };
        card.label_ids = vec!["l1".into()];
        // title + labels + info
        let lines = lines_at(&card, &[label], 40);
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn no_info_line_when_no_metadata() {
        let card = Card::new("Test".into());
        let lines = lines_at(&card, &[], 40);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn long_title_wraps() {
        let card = Card::new("one two three four".into());
        // width 9: "one two" / "three" / "four" → 3 title lines
        let lines = lines_at(&card, &[], 9);
        assert_eq!(lines.len(), 3);
    }

    /// Regression: `card_height` must equal what the widget renders.
    /// Overcount showed as a blank row above the bottom border (umlaut
    /// titles: the old title measurement counted bytes, so "ü" = 2);
    /// undercount clipped the info line. Sweep realistic content —
    /// multibyte, long words, many labels — across widths and require
    /// every inner row of the card to have content.
    #[test]
    fn no_blank_row_at_computed_card_height() {
        use crate::term_caps::TermCaps;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let names = [
            "Qualität",
            "Übergabe",
            "good first issue",
            "blocked",
            "needs-review",
            "documentation",
        ];
        let board_labels: Vec<Label> = names
            .iter()
            .map(|n| Label::new((*n).to_string(), LabelColor::Red))
            .collect();
        let titles = [
            "Überprüfung der Anmeldung",
            "Qualitätssicherung übergreifend",
            "Fix login flow",
            "A very long card title that wraps over multiple lines",
        ];
        for title in titles {
            let mut card = Card::new(title.into());
            card.label_ids = board_labels.iter().map(|l| l.id.clone()).collect();
            card.description = "x".into();
            card.checklist = vec![ChecklistItem {
                text: "a".into(),
                completed: false,
            }];
            for area_w in 8u16..60 {
                let inner_w = area_w.saturating_sub(2);
                let h = crate::ui::widgets::list_widget::card_height(
                    &card,
                    &board_labels,
                    inner_w,
                );
                let backend = TestBackend::new(area_w, h);
                let mut terminal = Terminal::new(backend).unwrap();
                terminal
                    .draw(|f| {
                        render(
                            f,
                            Rect::new(0, 0, area_w, h),
                            &card,
                            false,
                            false,
                            &board_labels,
                            Color::Cyan,
                            TermCaps::full(),
                        );
                    })
                    .unwrap();
                let buf = terminal.backend().buffer();
                for y in 1..h - 1 {
                    let row: String = (1..area_w - 1)
                        .map(|x| buf.cell((x, y)).unwrap().symbol())
                        .collect();
                    assert!(
                        !row.trim().is_empty(),
                        "blank card row: title {title:?} width {area_w} row {y}"
                    );
                }
            }
        }
    }

    #[test]
    fn multibyte_title_measured_in_chars_not_bytes() {
        // "Überprüfung" = 11 chars (13 bytes): fits width 11 on one line.
        let card = Card::new("Überprüfung".into());
        let lines = lines_at(&card, &[], 11);
        assert_eq!(lines.len(), 1);
    }
}
