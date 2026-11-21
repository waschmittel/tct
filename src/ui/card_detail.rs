use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::{App, AppMode};
use crate::insert::InsertSurface;

use super::markdown;
use super::widgets::card_widget;

/// Horizontal padding between the popup border and the description text.
const PAD: u16 = 2;
/// One space between the popup border and section headers / bodies.
const LEFT_PAD: u16 = 1;

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

    // Text wraps at WRAP_WIDTH, so a wider popup is wasted space:
    // cap at wrap width + horizontal padding + borders.
    let max_width = markdown::WRAP_WIDTH as u16 + 2 * PAD + 2;
    let editing_text = app.insert.as_ref().and_then(|h| {
        h.as_any()
            .downcast_ref::<crate::insert::markdown_editor::MarkdownEditor>()
            .map(|e| e.input.textarea.lines().join("\n"))
    });
    let description = editing_text.as_deref().unwrap_or(&card.description);
    // Tables are never wrapped: the popup grows to fit the widest one, up to the terminal.
    let table_width = markdown::widest_table_line(description.lines()) as u16 + 2 * PAD + 2;
    let screen_max = area.width.saturating_sub(2);
    let width = (area.width * 80 / 100)
        .max(40)
        .min(max_width)
        .max(table_width.min(screen_max))
        .min(screen_max);
    let height = (area.height * 80 / 100)
        .max(20)
        .min(area.height.saturating_sub(2));
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup);

    // Detect "editing description" via the active insert handler.
    let editing_desc_handler =
        if matches!(app.mode, AppMode::Insert | AppMode::Dialog | AppMode::Help) {
            app.insert.as_ref().and_then(|h| {
                h.as_any()
                    .downcast_ref::<crate::insert::markdown_editor::MarkdownEditor>()
            })
        } else {
            None
        };

    let title_display = if editing_desc_handler.is_some() {
        format!(" {} (editing description) ", card.title)
    } else {
        format!(" {} ", card.title)
    };
    let mut block = Block::default()
        .title(title_display)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent));
    let stamp_style = Style::default().fg(accent);
    if board.meta.show_updated_date
        && let Some(line) =
            card_widget::date_stamp(card.last_changed_at(), stamp_style, popup.width)
    {
        block = block.title_top(line);
    }
    if editing_desc_handler.is_some() && matches!(app.mode, AppMode::Insert | AppMode::Help) {
        let hint_text = if popup.width >= 24 {
            Some(" F1 / Ctrl+H:help ")
        } else if popup.width >= 12 {
            Some(" F1:help ")
        } else {
            None
        };
        if editing_desc_handler.is_some_and(|h| h.paste_mode()) {
            block = block.title_bottom(
                Line::from(Span::styled(
                    " PASTE MODE ",
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ))
                .right_aligned(),
            );
        }
        if let Some(text) = hint_text {
            block = block.title_bottom(Line::from(Span::styled(
                text,
                Style::default().fg(Color::DarkGray),
            )));
        }
    }
    if board.meta.show_created_date
        && let Some(line) = card_widget::date_stamp(card.created_at, stamp_style, popup.width)
    {
        block = block.title_bottom(line);
    }
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    if inner.height < 3 {
        return;
    }

    // If editing description, render the editor instead.
    if let Some(handler) = editing_desc_handler {
        render_description_editor(
            frame,
            popup,
            inner,
            handler,
            matches!(app.mode, AppMode::Insert),
            accent,
            app.caps.scrollbar_thumb(),
        );
        return;
    }

    // The popup is capped at WRAP_WIDTH + padding, so the inner width
    // already bounds the wrap width.
    let md_inner_width = inner.width.saturating_sub(PAD * 2);
    let desc_wrap_width = md_inner_width as usize;

    // Sections (headers, checklist, labels, due) sit one space off the
    // left and right borders so text doesn't touch the frame.
    let left = inner.x + LEFT_PAD;
    let body_w = inner.width.saturating_sub(LEFT_PAD * 2);

    // --- Section visibility ---
    // Empty sections are hidden to avoid wasted space. Exception for
    // discoverability: when every section is empty, all of them show
    // (with their placeholders).
    let resolved = card.resolved_labels(&board.meta.labels);
    let show_all = card.description.is_empty()
        && card.checklist.is_empty()
        && resolved.is_empty()
        && card.due_date.is_none();
    let show_desc = show_all || !card.description.is_empty();
    let show_checklist = show_all || !card.checklist.is_empty();
    let show_labels = show_all || !resolved.is_empty();
    let show_due = show_all || card.due_date.is_some();

    // --- Build content for each visible section ---
    let desc_lines: Vec<Line<'static>> = if !show_desc {
        Vec::new()
    } else if card.description.is_empty() {
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

    // Long items wrap; continuation lines align under the item text, so an
    // item can span several lines. `checklist_item_offsets` maps item index
    // to (first line, line count) for selection-driven scrolling.
    let mut checklist_item_offsets: Vec<(usize, usize)> = Vec::new();
    let checklist_lines: Vec<Line<'static>> = if !show_checklist {
        Vec::new()
    } else if card.checklist.is_empty() {
        vec![Line::from(Span::styled(
            "  (no items)",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        let mut lines = Vec::new();
        for (ii, item) in card.checklist.iter().enumerate() {
            let is_active = ii == board.detail_item_idx;
            let check = if item.completed {
                app.caps.check_mark()
            } else {
                " "
            };
            let style = if is_active {
                Style::default().fg(accent).add_modifier(Modifier::BOLD)
            } else if item.completed {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };
            let prefix = if is_active { "» " } else { "  " };
            let wrapped = markdown::wrap_spans_with_indent(
                vec![Span::styled(
                    format!("{prefix}[{check}] {}", item.text),
                    style,
                )],
                body_w as usize,
                6, // width of "» [x] " — continuation lines align under the text
            );
            checklist_item_offsets.push((lines.len(), wrapped.len()));
            lines.extend(wrapped);
        }
        lines
    };

    let labels_lines: Vec<Line<'static>> = if !show_labels {
        Vec::new()
    } else if resolved.is_empty() {
        vec![Line::from(Span::styled(
            "  (no labels)",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        // Same look as on the card in the list view: [name] chips on the
        // label's color, side by side, wrapped at chip boundaries.
        super::widgets::labels::label_lines(&resolved, body_w as usize, false)
    };

    let due_lines: Vec<Line<'static>> = if !show_due {
        Vec::new()
    } else if let Some(due) = card.due_date {
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
    // Fixed chrome: 1 (top blank) + 1 header per visible section (except description)
    // + a 3-line divider between each pair of adjacent visible sections.
    let top_pad: u16 = 1;
    let header_h: u16 = 1;
    let div_h: u16 = 3;

    let labels_full = labels_lines.len() as u16;
    let due_full = due_lines.len() as u16;

    let visible_sections = [show_desc, show_checklist, show_labels, show_due]
        .iter()
        .filter(|s| **s)
        .count() as u16;
    let div_total = visible_sections.saturating_sub(1) * div_h;

    // Reserve full space for labels + due; they're small.
    let bottom_h = (if show_labels {
        header_h + labels_full
    } else {
        0
    }) + (if show_due { header_h + due_full } else { 0 });
    let fixed_top = top_pad + (if show_checklist { header_h } else { 0 });
    let available = inner
        .height
        .saturating_sub(fixed_top + div_total + bottom_h);

    let desc_full = desc_lines.len() as u16;
    let ck_full = checklist_lines.len() as u16;
    let (desc_h, ck_h) = split_available(desc_full, ck_full, available);

    // Clamp description scroll
    let desc_max_scroll = desc_full.saturating_sub(desc_h);
    let desc_scroll = (board.detail_scroll as u16).min(desc_max_scroll);
    // Report the effective max back to the input layer so scroll keys clamp
    // against what is actually rendered.
    board.detail_max_scroll.set(desc_max_scroll as usize);

    // Auto-scroll checklist to keep the selected item's last line visible
    let ck_max_scroll = ck_full.saturating_sub(ck_h);
    let ck_scroll = if ck_h == 0 {
        0
    } else {
        let (start, h) = checklist_item_offsets
            .get(board.detail_item_idx)
            .copied()
            .unwrap_or((0, 1));
        ((start + h) as u16).saturating_sub(ck_h).min(ck_max_scroll)
    };

    // --- Render sections sequentially ---
    let mut y = inner.y;

    // Top blank
    y += top_pad;

    // Divider drawn before each section after the first; the one following
    // the description is heavier (═).
    let mut divider: Option<&str> = None;

    if show_desc {
        // Description body (scrollbar on the right when it overflows)
        if desc_h > 0 && y < inner.y + inner.height {
            let desc_area = Rect::new(inner.x + PAD, y, md_inner_width, desc_h);
            let desc_paragraph = Paragraph::new(desc_lines.clone()).scroll((desc_scroll, 0));
            frame.render_widget(desc_paragraph, desc_area);
            if desc_full > desc_h {
                let bar_area = Rect::new(popup.x, y, popup.width, desc_h);
                // positions = max_scroll + 1 so the thumb reaches the track
                // bottom at max scroll (ratatui puts it there only when
                // position == content_length - 1).
                render_scrollbar(
                    frame,
                    bar_area,
                    (desc_max_scroll + 1) as usize,
                    desc_scroll as usize,
                    accent,
                    app.caps.scrollbar_thumb(),
                );
            }
            y += desc_h;
        }

        divider = Some("═");
    }

    if show_checklist {
        if let Some(glyph) = divider.take()
            && y + div_h <= inner.y + inner.height
        {
            render_divider(frame, inner, y, glyph);
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
            let header_area = Rect::new(left, y, body_w, header_h);
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    ck_header_text,
                    Style::default().fg(accent).add_modifier(Modifier::BOLD),
                ))),
                header_area,
            );
            y += header_h;
        }

        // Checklist body (sliced)
        if ck_h > 0 && y < inner.y + inner.height {
            let start = ck_scroll as usize;
            let end = (start + ck_h as usize).min(checklist_lines.len());
            let visible: Vec<Line<'static>> = checklist_lines[start..end].to_vec();
            let ck_area = Rect::new(left, y, body_w, ck_h);
            frame.render_widget(Paragraph::new(visible), ck_area);
            y += ck_h;
        }

        divider = Some("─");
    }

    if show_labels {
        if let Some(glyph) = divider.take()
            && y + div_h <= inner.y + inner.height
        {
            render_divider(frame, inner, y, glyph);
            y += div_h;
        }

        // Labels header + body
        if y < inner.y + inner.height {
            let header_area = Rect::new(left, y, body_w, header_h);
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "Labels",
                    Style::default().fg(accent).add_modifier(Modifier::BOLD),
                ))),
                header_area,
            );
            y += header_h;
        }
        if labels_full > 0 && y < inner.y + inner.height {
            let remaining = (inner.y + inner.height).saturating_sub(y);
            let h = labels_full.min(remaining);
            let labels_area = Rect::new(left, y, body_w, h);
            frame.render_widget(Paragraph::new(labels_lines), labels_area);
            y += h;
        }

        divider = Some("─");
    }

    if show_due {
        if let Some(glyph) = divider.take()
            && y + div_h <= inner.y + inner.height
        {
            render_divider(frame, inner, y, glyph);
            y += div_h;
        }

        // Due date header + body
        if y < inner.y + inner.height {
            let header_area = Rect::new(left, y, body_w, header_h);
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "Due Date",
                    Style::default().fg(accent).add_modifier(Modifier::BOLD),
                ))),
                header_area,
            );
            y += header_h;
        }
        if due_full > 0 && y < inner.y + inner.height {
            let remaining = (inner.y + inner.height).saturating_sub(y);
            let h = due_full.min(remaining);
            let due_area = Rect::new(left, y, body_w, h);
            frame.render_widget(Paragraph::new(due_lines), due_area);
        }
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

fn render_divider(frame: &mut Frame, inner: Rect, y: u16, glyph: &str) {
    let divider_area = Rect::new(inner.x, y, inner.width, 3);
    let lines = vec![
        Line::raw(""),
        Line::from(Span::styled(
            glyph.repeat(inner.width as usize),
            Style::default().fg(Color::DarkGray),
        )),
        Line::raw(""),
    ];
    frame.render_widget(Paragraph::new(lines), divider_area);
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
    popup: Rect,
    inner: Rect,
    handler: &crate::insert::markdown_editor::MarkdownEditor,
    show_find_cursor: bool,
    accent: Color,
    thumb_symbol: &str,
) {
    let textarea = &handler.input.textarea;
    let find = handler.find_state();
    let bar_h: u16 = match find {
        Some(f) => {
            let needed = if f.replace_mode { 3 } else { 2 };
            if inner.height >= needed + 2 {
                needed
            } else {
                0
            }
        }
        None => 0,
    };
    let editor_h = inner.height.saturating_sub(bar_h);

    // Match the rendered description view's wrap width (inner width minus
    // padding). The text sits one space off the left border. The textarea
    // keeps its last column free for the caret, so it gets one extra to
    // wrap where the view does.
    let width = inner.width.saturating_sub(PAD * 2) + 1;
    let editor_area = Rect::new(inner.x + LEFT_PAD, inner.y, width, editor_h);
    frame.render_widget(textarea, editor_area);

    // Scrollbar tracks the cursor's data line — the textarea keeps the
    // cursor in view, so this follows the viewport closely enough.
    // Thumb is drawn over the right border between the corners.
    let total = textarea.lines().len();
    if total > editor_h as usize {
        let (cursor_row, _) = textarea.cursor();
        let track = Rect::new(popup.x, popup.y + 1, popup.width, editor_h);
        render_scrollbar(frame, track, total, cursor_row, accent, thumb_symbol);
    }

    if let Some(find) = find
        && bar_h > 0
    {
        let bar_area = Rect::new(inner.x, inner.y + editor_h, inner.width, bar_h);
        render_find_bar(frame, bar_area, find, show_find_cursor, accent);
    }
}

fn chip_style(focused: bool, active: bool, accent: Color, inactive: Color) -> Style {
    if focused {
        Style::default()
            .fg(Color::Black)
            .bg(accent)
            .add_modifier(Modifier::BOLD)
    } else if active {
        Style::default().fg(accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(inactive)
    }
}

fn render_find_bar(
    frame: &mut Frame,
    area: Rect,
    find: &crate::insert::markdown_editor::FindState,
    show_cursor: bool,
    accent: Color,
) {
    use crate::insert::markdown_editor::FindFocus;

    // Top separator row
    let sep = Line::from(Span::styled(
        "─".repeat(area.width as usize),
        Style::default().fg(Color::DarkGray),
    ));
    frame.render_widget(
        Paragraph::new(sep),
        Rect::new(area.x, area.y, area.width, 1),
    );

    let left = area.x + LEFT_PAD;
    let row_w = area.width.saturating_sub(LEFT_PAD * 2);
    if row_w < 10 {
        return;
    }

    // --- Find row ---
    let find_y = area.y + 1;
    let find_label = if find.replace_mode {
        "Find:    "
    } else {
        "Find: "
    };
    let label_style = if find.focus == FindFocus::Query {
        Style::default().fg(accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let mut right_spans: Vec<Span<'static>> = Vec::new();
    let mut right_w: u16 = 0;
    if let Some(status) = find.status_label() {
        let status_style = if find.regex_error || find.matches.is_empty() {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        right_w += status.chars().count() as u16 + 2;
        right_spans.push(Span::styled(status, status_style));
        right_spans.push(Span::raw("  "));
    }
    right_spans.push(Span::styled(
        "[Aa]",
        chip_style(
            find.focus == FindFocus::CaseSensitive,
            find.case_sensitive,
            accent,
            Color::DarkGray,
        ),
    ));
    right_spans.push(Span::raw(" "));
    right_spans.push(Span::styled(
        "[.*]",
        chip_style(
            find.focus == FindFocus::Regex,
            find.use_regex,
            accent,
            Color::DarkGray,
        ),
    ));
    right_w += 9; // "[Aa] [.*]"

    render_bar_input_row(
        frame,
        Rect::new(left, find_y, row_w, 1),
        find_label,
        label_style,
        &find.query.buffer,
        find.query.cursor,
        show_cursor && find.focus == FindFocus::Query,
        right_spans,
        right_w,
    );

    // --- Replace row ---
    if find.replace_mode && area.height >= 3 {
        let rep_y = area.y + 2;
        let rep_label = "Replace: ";
        let rep_label_style = if find.focus == FindFocus::Replace {
            Style::default().fg(accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let rep_right = vec![
            Span::styled(
                "[Replace]",
                chip_style(
                    find.focus == FindFocus::ReplaceOne,
                    false,
                    accent,
                    Color::Gray,
                ),
            ),
            Span::raw(" "),
            Span::styled(
                "[All]",
                chip_style(
                    find.focus == FindFocus::ReplaceAll,
                    false,
                    accent,
                    Color::Gray,
                ),
            ),
        ];
        let rep_right_w = 15; // "[Replace] [All]"

        render_bar_input_row(
            frame,
            Rect::new(left, rep_y, row_w, 1),
            rep_label,
            rep_label_style,
            &find.replace.buffer,
            find.replace.cursor,
            show_cursor && find.focus == FindFocus::Replace,
            rep_right,
            rep_right_w,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn render_bar_input_row(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    label_style: Style,
    buffer: &str,
    cursor: usize,
    place_cursor: bool,
    right_spans: Vec<Span<'static>>,
    right_w: u16,
) {
    let label_w = label.chars().count() as u16;
    let show_right = area.width > label_w + right_w + 2;
    let input_w = if show_right {
        area.width.saturating_sub(label_w + right_w + 1)
    } else {
        area.width.saturating_sub(label_w)
    };

    // Label
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(label.to_string(), label_style))),
        Rect::new(area.x, area.y, label_w.min(area.width), 1),
    );

    // Horizontally-scrolled input text
    if input_w > 0 {
        let visible_w = input_w as usize;
        let cursor_char_idx = buffer[..cursor].chars().count();
        let scroll = if cursor_char_idx >= visible_w {
            cursor_char_idx - visible_w + 1
        } else {
            0
        };
        let visible: String = buffer.chars().skip(scroll).take(visible_w).collect();
        let input_x = area.x + label_w;
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                visible,
                Style::default().fg(Color::White),
            ))),
            Rect::new(input_x, area.y, input_w, 1),
        );
        if place_cursor {
            let cx = input_x + (cursor_char_idx - scroll) as u16;
            if cx < input_x + input_w {
                frame.set_cursor_position((cx, area.y));
            }
        }
    }

    // Right-aligned status + chips
    if show_right {
        let rx = area.x + area.width.saturating_sub(right_w);
        frame.render_widget(
            Paragraph::new(Line::from(right_spans)),
            Rect::new(rx, area.y, right_w, 1),
        );
    }
}

/// `positions` is the number of thumb positions: the thumb sits at the track
/// bottom when `position == positions - 1`.
fn render_scrollbar(
    frame: &mut Frame,
    area: Rect,
    positions: usize,
    position: usize,
    accent: Color,
    thumb_symbol: &str,
) {
    use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};
    let mut state = ScrollbarState::new(positions)
        .position(position)
        .viewport_content_length(area.height as usize);
    frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(None)
            .thumb_symbol(thumb_symbol)
            .style(Style::default().fg(accent)),
        area,
        &mut state,
    );
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
