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
    stamps: DateStamps,
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
    let stamp_style = if dimmed {
        Style::default().fg(Color::DarkGray)
    } else if selected {
        Style::default().fg(accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    if stamps.updated
        && let Some(line) = date_stamp(card.last_changed_at(), stamp_style, area.width)
    {
        block = block.title_top(line);
    }
    if stamps.created
        && let Some(line) = date_stamp(card.created_at, stamp_style, area.width)
    {
        block = block.title_bottom(line);
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
        let label = due_label(*due, today);
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

/// Which date stamps a card frame carries, per the board's settings.
#[derive(Clone, Copy, Default)]
pub struct DateStamps {
    pub created: bool,
    pub updated: bool,
}

/// A date for a frame corner (right-aligned title), or `None` when `frame_width`
/// has no room for it beside the border corners.
pub fn date_stamp(
    at: chrono::DateTime<chrono::Utc>,
    style: Style,
    frame_width: u16,
) -> Option<Line<'static>> {
    let today = chrono::Local::now().date_naive();
    let date = short_date(at.with_timezone(&chrono::Local).date_naive(), today);
    let text = format!(" {date} ");
    (text.chars().count() as u16 + 2 <= frame_width)
        .then(|| Line::from(Span::styled(text, style)).right_aligned())
}

/// `mm-dd`, or `yyyy-mm-dd` when the year is not the current one.
pub fn short_date(date: chrono::NaiveDate, today: chrono::NaiveDate) -> String {
    use chrono::Datelike;
    if date.year() == today.year() {
        date.format("%m-%d").to_string()
    } else {
        date.format("%Y-%m-%d").to_string()
    }
}

/// Due date as shown on the card: [`short_date`] plus how far away it is
/// when that is near or past.
fn due_label(due: chrono::NaiveDate, today: chrono::NaiveDate) -> String {
    let date = short_date(due, today);
    match (due - today).num_days() {
        days if days < 0 => format!("{date} (-{}d)", -days),
        0 => format!("{date} (today)"),
        days if days <= 3 => format!("{date} ({days}d)"),
        _ => date,
    }
}

#[cfg(test)]
mod due_label_tests {
    use super::{due_label, short_date};
    use chrono::NaiveDate;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn short_date_drops_only_the_current_year() {
        assert_eq!(short_date(d(2026, 3, 4), d(2026, 10, 16)), "03-04");
        assert_eq!(short_date(d(2025, 3, 4), d(2026, 10, 16)), "2025-03-04");
    }

    #[test]
    fn current_year_omits_the_year() {
        assert_eq!(due_label(d(2026, 12, 24), d(2026, 10, 16)), "12-24");
    }

    #[test]
    fn other_years_show_the_year() {
        assert_eq!(due_label(d(2027, 1, 5), d(2026, 10, 16)), "2027-01-05");
        assert_eq!(due_label(d(2025, 12, 31), d(2026, 10, 16)), "2025-12-31 (-289d)");
    }

    #[test]
    fn nearby_dates_keep_the_countdown_across_a_year_boundary() {
        assert_eq!(due_label(d(2027, 1, 1), d(2026, 12, 30)), "2027-01-01 (2d)");
        assert_eq!(due_label(d(2026, 10, 16), d(2026, 10, 16)), "10-16 (today)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::card::ChecklistItem;
    use crate::model::label::LabelColor;
    use chrono::TimeZone;
    use ratatui::{Terminal, backend::TestBackend};

    /// Frame rows of a 30x5 card with fixed (past-year) dates.
    fn frame_rows(stamps: DateStamps, width: u16) -> Vec<String> {
        let mut card = Card::new("Task".into());
        card.created_at = chrono::Utc.with_ymd_and_hms(2020, 1, 2, 12, 0, 0).unwrap();
        card.log("Edited");
        card.history.last_mut().unwrap().at = chrono::Utc.with_ymd_and_hms(2021, 3, 4, 12, 0, 0).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(width, 5)).unwrap();
        terminal
            .draw(|f| render(f, f.area(), &card, false, false, &[], Color::Cyan, TermCaps::full(), stamps))
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        (0..5)
            .map(|y| (0..width).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>())
            .collect()
    }

    #[test]
    fn selected_card_stamps_use_the_accent_color() {
        let mut card = Card::new("Task".into());
        card.created_at = chrono::Utc.with_ymd_and_hms(2020, 1, 2, 12, 0, 0).unwrap();
        let accent = Color::Magenta;
        let fg_of_stamp = |selected: bool| {
            let mut terminal = Terminal::new(TestBackend::new(30, 5)).unwrap();
            terminal
                .draw(|f| {
                    render(
                        f, f.area(), &card, selected, false, &[], accent, TermCaps::full(),
                        DateStamps { created: true, updated: false },
                    )
                })
                .unwrap();
            // The stamp's last digit, just left of " ┘".
            terminal.backend().buffer()[(27, 4)].fg
        };
        assert_eq!(fg_of_stamp(true), accent);
        assert_eq!(fg_of_stamp(false), Color::Gray);
    }

    #[test]
    fn no_stamps_by_default() {
        let rows = frame_rows(DateStamps::default(), 30);
        assert!(rows.iter().all(|r| !r.contains("20")), "{rows:?}");
    }

    #[test]
    fn updated_date_sits_in_the_top_right_of_the_frame() {
        let rows = frame_rows(DateStamps { created: false, updated: true }, 30);
        assert!(rows[0].trim_end().ends_with("┐") && rows[0].contains(" 2021-03-04 ┐"), "{rows:?}");
        assert!(!rows[4].contains("2020"), "{rows:?}");
    }

    #[test]
    fn created_date_sits_in_the_bottom_right_of_the_frame() {
        let rows = frame_rows(DateStamps { created: true, updated: false }, 30);
        assert!(rows[4].contains(" 2020-01-02 ┘"), "{rows:?}");
        assert!(!rows[0].contains("2021"), "{rows:?}");
    }

    #[test]
    fn both_stamps_together() {
        let rows = frame_rows(DateStamps { created: true, updated: true }, 30);
        assert!(rows[0].contains("2021-03-04") && rows[4].contains("2020-01-02"), "{rows:?}");
    }

    #[test]
    fn stamps_are_skipped_when_the_card_is_too_narrow() {
        let rows = frame_rows(DateStamps { created: true, updated: true }, 12);
        assert!(rows.iter().all(|r| !r.contains("20")), "{rows:?}");
    }

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
                            DateStamps::default(),
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
