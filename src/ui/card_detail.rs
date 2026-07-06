use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{App, AppMode};
use crate::insert::InsertSurface;

use super::markdown;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let board = match app.board() {
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

    // Detect "editing description" via the active insert handler.
    let editing_desc_handler = if matches!(app.mode, AppMode::Insert) {
        app.insert.as_ref().and_then(|h| {
            h.as_any()
                .downcast_ref::<crate::insert::markdown_editor::MarkdownEditor>()
        })
    } else {
        None
    };
    let block = Block::default()
        .title(title_display)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    if inner.height < 3 {
        return;
    }

    // If editing description, render the editor instead.
    if let Some(handler) = editing_desc_handler {
        render_description_editor(
            frame,
            inner,
            &handler.input.textarea,
            handler.input.scroll,
            accent,
        );
        return;
    }

    let pad: u16 = 2;
    let md_inner_width = inner.width.saturating_sub(pad * 2);
    let desc_wrap_width = (md_inner_width as usize).min(markdown::WRAP_WIDTH);

    // --- Build content for each section ---
    let desc_lines: Vec<Line<'static>> = if card.description.is_empty() {
        vec![Line::from(Span::styled(
            "(no description)",
            Style::default().fg(Color::DarkGray),
        ))]
    } else if desc_wrap_width == 0 {
        Vec::new()
    } else {
        markdown::MarkdownRenderer::new(&card.description, desc_wrap_width, accent)
            .render()
            .lines()
            .to_vec()
    };

    let checklist_lines: Vec<Line<'static>> = if card.checklist.is_empty() {
        vec![Line::from(Span::styled(
            "  (no items)",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        card.checklist
            .iter()
            .enumerate()
            .map(|(ii, item)| {
                let is_active = ii == board.detail_item_idx;
                let check = if item.completed { "✓" } else { " " };
                let style = if is_active {
                    Style::default().fg(accent).add_modifier(Modifier::BOLD)
                } else if item.completed {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default()
                };
                let prefix = if is_active { "» " } else { "  " };
                Line::from(Span::styled(
                    format!("{prefix}[{check}] {}", item.text),
                    style,
                ))
            })
            .collect()
    };

    let resolved = card.resolved_labels(&board.meta.labels);
    let labels_lines: Vec<Line<'static>> = if resolved.is_empty() {
        vec![Line::from(Span::styled(
            "  (no labels)",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        resolved
            .iter()
            .map(|label| {
                Line::from(Span::styled(
                    format!("  ● {}", label.name),
                    Style::default()
                        .fg(Color::Black)
                        .bg(label.color.to_ratatui_color()),
                ))
            })
            .collect()
    };

    let due_lines: Vec<Line<'static>> = if let Some(due) = card.due_date {
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
                format!("  {}", due.format("%Y-%m-%d")),
                Style::default().fg(Color::White),
            )),
            Line::from(Span::styled(
                format!("  {status}"),
                Style::default().fg(color),
            )),
        ]
    } else {
        vec![Line::from(Span::styled(
            "  (no due date)",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    // --- Compute heights with caps for scrollable sections ---
    // Fixed chrome:
    //  1 (top blank) + 1 (desc header)
    //  + 3 (divider ═)
    //  + 1 (checklist header)
    //  + 3 (divider ─)
    //  + 1 (labels header)
    //  + 3 (divider ─)
    //  + 1 (due header)
    // = 14
    let top_pad: u16 = 1;
    let desc_header_h: u16 = 1;
    let ck_header_h: u16 = 1;
    let labels_header_h: u16 = 1;
    let due_header_h: u16 = 1;
    let div_h: u16 = 3;

    let labels_full = labels_lines.len() as u16;
    let due_full = due_lines.len() as u16;

    // Reserve full space for labels + due; they're small.
    let bottom_h = div_h + labels_header_h + labels_full + div_h + due_header_h + due_full;
    let fixed_top = top_pad + desc_header_h + div_h + ck_header_h;
    let available = inner.height.saturating_sub(fixed_top + bottom_h);

    let desc_full = desc_lines.len() as u16;
    let ck_full = checklist_lines.len() as u16;
    let (desc_h, ck_h) = split_available(desc_full, ck_full, available);

    // Clamp description scroll
    let desc_max_scroll = desc_full.saturating_sub(desc_h);
    let desc_scroll = (board.detail_scroll as u16).min(desc_max_scroll);

    // Auto-scroll checklist to keep selected item visible
    let ck_max_scroll = ck_full.saturating_sub(ck_h);
    let ck_scroll = if ck_h == 0 {
        0
    } else {
        let idx = board.detail_item_idx as u16;
        idx.saturating_sub(ck_h.saturating_sub(1)).min(ck_max_scroll)
    };

    // --- Render sections sequentially ---
    let mut y = inner.y;

    // Top blank
    y += top_pad;

    // Description header (with optional scroll hint)
    let desc_header_text = if desc_full > desc_h {
        let total_visual_lines = desc_full.max(1);
        let bottom = (desc_scroll + desc_h).min(total_visual_lines);
        format!("Description  [{}-{} / {}]", desc_scroll + 1, bottom, total_visual_lines)
    } else {
        "Description".to_string()
    };
    if y < inner.y + inner.height {
        let header_area = Rect::new(inner.x, y, inner.width, desc_header_h);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                desc_header_text,
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ))),
            header_area,
        );
        y += desc_header_h;
    }

    // Description body
    if desc_h > 0 && y < inner.y + inner.height {
        let desc_area = Rect::new(inner.x + pad, y, md_inner_width, desc_h);
        let desc_paragraph =
            Paragraph::new(desc_lines.clone()).scroll((desc_scroll, 0));
        frame.render_widget(desc_paragraph, desc_area);
        y += desc_h;
    }

    // Divider (blank + ═ + blank)
    if y + div_h <= inner.y + inner.height {
        let divider_area = Rect::new(inner.x, y, inner.width, div_h);
        let lines = vec![
            Line::raw(""),
            Line::from(Span::styled(
                "═".repeat(inner.width as usize),
                Style::default().fg(Color::DarkGray),
            )),
            Line::raw(""),
        ];
        frame.render_widget(Paragraph::new(lines), divider_area);
        y += div_h;
    }

    // Checklist header
    let (done, total) = card.checklist_progress().unwrap_or((0, 0));
    let ck_header_text = if total > 0 {
        if ck_full > ck_h {
            format!("Checklist [{done}/{total}]  ({} hidden)", ck_full - ck_h)
        } else {
            format!("Checklist [{done}/{total}]")
        }
    } else {
        "Checklist".to_string()
    };
    if y < inner.y + inner.height {
        let header_area = Rect::new(inner.x, y, inner.width, ck_header_h);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                ck_header_text,
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ))),
            header_area,
        );
        y += ck_header_h;
    }

    // Checklist body (sliced)
    if ck_h > 0 && y < inner.y + inner.height {
        let start = ck_scroll as usize;
        let end = (start + ck_h as usize).min(checklist_lines.len());
        let visible: Vec<Line<'static>> = checklist_lines[start..end].to_vec();
        let ck_area = Rect::new(inner.x, y, inner.width, ck_h);
        frame.render_widget(Paragraph::new(visible), ck_area);
        y += ck_h;
    }

    // Divider
    if y + div_h <= inner.y + inner.height {
        let divider_area = Rect::new(inner.x, y, inner.width, div_h);
        let lines = vec![
            Line::raw(""),
            Line::from(Span::styled(
                "─".repeat(inner.width as usize),
                Style::default().fg(Color::DarkGray),
            )),
            Line::raw(""),
        ];
        frame.render_widget(Paragraph::new(lines), divider_area);
        y += div_h;
    }

    // Labels header + body
    if y < inner.y + inner.height {
        let header_area = Rect::new(inner.x, y, inner.width, labels_header_h);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "Labels",
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ))),
            header_area,
        );
        y += labels_header_h;
    }
    if labels_full > 0 && y < inner.y + inner.height {
        let remaining = (inner.y + inner.height).saturating_sub(y);
        let h = labels_full.min(remaining);
        let labels_area = Rect::new(inner.x, y, inner.width, h);
        frame.render_widget(Paragraph::new(labels_lines), labels_area);
        y += h;
    }

    // Divider
    if y + div_h <= inner.y + inner.height {
        let divider_area = Rect::new(inner.x, y, inner.width, div_h);
        let lines = vec![
            Line::raw(""),
            Line::from(Span::styled(
                "─".repeat(inner.width as usize),
                Style::default().fg(Color::DarkGray),
            )),
            Line::raw(""),
        ];
        frame.render_widget(Paragraph::new(lines), divider_area);
        y += div_h;
    }

    // Due date header + body
    if y < inner.y + inner.height {
        let header_area = Rect::new(inner.x, y, inner.width, due_header_h);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "Due Date",
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ))),
            header_area,
        );
        y += due_header_h;
    }
    if due_full > 0 && y < inner.y + inner.height {
        let remaining = (inner.y + inner.height).saturating_sub(y);
        let h = due_full.min(remaining);
        let due_area = Rect::new(inner.x, y, inner.width, h);
        frame.render_widget(Paragraph::new(due_lines), due_area);
    }

    // Input dialogs rendered on top — driven by the active insert handler
    // when its surface is `CardDetail`.
    if matches!(&app.mode, AppMode::Insert)
        && let Some(handler) = app.insert.as_ref()
        && handler.surface() == InsertSurface::CardDetail
    {
        if let Some(dp) = handler
            .as_any()
            .downcast_ref::<crate::insert::date_picker::DatePicker>()
        {
            super::widgets::date_picker::render(
                frame,
                popup,
                &dp.buffer,
                dp.cursor,
                dp.picker_date,
                accent,
            );
        } else if let (Some(buf), Some(cursor)) = (handler.line_buffer(), handler.line_cursor()) {
            render_input_dialog(frame, popup, handler.title(), buf, cursor, accent);
        }
    }
}

fn split_available(desc_full: u16, ck_full: u16, avail: u16) -> (u16, u16) {
    if avail == 0 {
        return (0, 0);
    }
    if desc_full + ck_full <= avail {
        return (desc_full, ck_full);
    }
    let half = avail / 2;
    let other = avail - half;
    if desc_full <= half {
        return (desc_full, avail - desc_full);
    }
    if ck_full <= half {
        return (avail - ck_full, ck_full);
    }
    // Both bigger than half: give bigger of the two slightly more
    if desc_full >= ck_full {
        (other, half)
    } else {
        (half, other)
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
        .border_style(Style::default().fg(accent))
        .title(Line::from(Span::styled(
            " Edit Description ",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        )));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let visible_height = inner.height as usize;
    let lines: Vec<String> = textarea.lines().iter().map(|s| s.to_string()).collect();
    let ratatui_textarea::DataCursor(cursor_row, cursor_col) = textarea.cursor();

    let wrap_width = markdown::WRAP_WIDTH.min(inner.width as usize);

    let rendered = markdown::MarkdownRenderer::from_lines(&lines, wrap_width, accent).render();
    let visual_lines = rendered.lines();
    let (cursor_visual_row_u16, cursor_visual_col_u16) = rendered.cursor_at(cursor_row, cursor_col);
    let cursor_visual_row = cursor_visual_row_u16 as usize;
    let cursor_visual_col = cursor_visual_col_u16 as usize;

    // Adjust scroll to keep cursor visible
    let scroll = if cursor_visual_row < editor_scroll {
        cursor_visual_row
    } else if cursor_visual_row >= editor_scroll + visible_height {
        cursor_visual_row - visible_height + 1
    } else {
        editor_scroll
    };

    let end = (scroll + visible_height).min(visual_lines.len());
    let start = scroll.min(end);

    for (vi, idx) in (start..end).enumerate() {
        let vline = &visual_lines[idx];
        let src_li = rendered.src_row_for(idx).unwrap_or(0);

        let y = inner.y + vi as u16;
        let line_area = Rect::new(inner.x, y, inner.width, 1);

        let line_spans: Vec<Span<'static>> = vline.spans.to_vec();

        if src_li == cursor_row && idx == cursor_visual_row {
            frame.render_widget(
                Paragraph::new(Line::from(line_spans))
                    .style(Style::default().bg(Color::Rgb(30, 30, 40))),
                line_area,
            );
        } else {
            frame.render_widget(Paragraph::new(Line::from(line_spans)), line_area);
        }
    }

    if cursor_visual_row >= start && cursor_visual_row < end {
        let cx = inner.x + (cursor_visual_col as u16).min(inner.width.saturating_sub(1));
        let cy = inner.y + (cursor_visual_row - start) as u16;
        frame.set_cursor_position((cx, cy));
    }
}

fn render_input_dialog(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    input: &str,
    cursor: usize,
    accent: Color,
) {
    let width = 50u16.min(area.width.saturating_sub(2));
    let height = 3u16;
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let dialog = Rect::new(x, y, width, height);

    frame.render_widget(Clear, dialog);

    let block = Block::default()
        .title(format!(" {title} "))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent));
    let inner = block.inner(dialog);
    frame.render_widget(block, dialog);

    let visible_w = inner.width as usize;
    let cursor_char_idx = input[..cursor].chars().count();
    let scroll = if cursor_char_idx >= visible_w {
        cursor_char_idx - visible_w + 1
    } else {
        0
    };

    let visible: String = input.chars().skip(scroll).take(visible_w).collect();
    frame.render_widget(Paragraph::new(visible), inner);

    let cx = inner.x + (cursor_char_idx - scroll) as u16;
    if cx < inner.x + inner.width {
        frame.set_cursor_position((cx, inner.y));
    }
}
