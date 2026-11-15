pub mod board_selector;
pub mod board_view;
pub mod card_detail;
pub mod dialog;
pub mod markdown;
pub mod search_bar;
#[cfg(test)]
mod snapshot_tests;
pub mod status_bar;
pub mod theme;
pub mod widgets;

use ratatui::Frame;

use crate::app::{App, AppMode};
use crate::insert::InsertSurface;

/// Resolve the surface of the currently active `InsertHandler`, if any.
fn insert_surface(app: &App) -> Option<InsertSurface> {
    app.insert.as_ref().map(|h| h.surface())
}

/// Whether the active dialog was opened from the card detail: its return
/// mode is the detail, or it sits over a live card-detail insert (the
/// discard prompt over the description editor).
fn dialog_over_card_detail(app: &App) -> bool {
    matches!(app.previous_mode, Some(AppMode::CardDetail))
        || matches!(insert_surface(app), Some(InsertSurface::CardDetail))
}

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // 1. Determine and render the base background layer
    let effective_mode = if let AppMode::Help = app.mode {
        app.previous_mode.as_ref().unwrap_or(&app.mode)
    } else {
        &app.mode
    };

    let dialog_over_selector = matches!(&app.mode, AppMode::Dialog)
        && app
            .dialog
            .as_ref()
            .map(|d| {
                matches!(
                    d.background(),
                    crate::dialog::DialogBackground::BoardSelector
                )
            })
            .unwrap_or(false);

    let insert_over_selector = matches!(&app.mode, AppMode::Insert)
        && matches!(insert_surface(app), Some(InsertSurface::BoardSelector));

    let is_board_selector_base = app.editor.is_none()
        || matches!(effective_mode, AppMode::BoardSelector)
        || dialog_over_selector
        || insert_over_selector;

    if is_board_selector_base {
        board_selector::render(frame, area, app);
    } else {
        board_view::render(frame, area, app);
    }

    // 2. Render Overlays
    match &app.mode {
        AppMode::Help => {
            // Help pops over whatever it was opened from: the card detail
            // (directly, over its editor, or under a dialog) and a live
            // dialog stay visible underneath.
            let over_card_detail =
                matches!(effective_mode, AppMode::CardDetail) || dialog_over_card_detail(app);
            if over_card_detail && !dialog_over_selector {
                card_detail::render(frame, area, app);
            }
            if app.dialog.is_some() {
                dialog::render(frame, area, app);
            }
            render_help(frame, area, app);
        }
        AppMode::Command => {
            search_bar::render(frame, area, app);
        }
        AppMode::CardDetail => {
            card_detail::render(frame, area, app);
        }
        AppMode::Insert => {
            // Card-detail inserts render the popup themselves (the
            // handler's render is invoked inside card_detail::render).
            if matches!(insert_surface(app), Some(InsertSurface::CardDetail)) {
                card_detail::render(frame, area, app);
            }
            // BoardView/BoardSelector inserts are drawn by the
            // respective background views via consulting `app.insert`.
        }
        AppMode::Dialog => {
            // A dialog opened from the card detail pops over it, like the
            // date picker does.
            if dialog_over_card_detail(app) && !dialog_over_selector {
                card_detail::render(frame, area, app);
            }
            dialog::render(frame, area, app);
        }
        _ => {}
    }

    // 3. Transient status toast, top-right, above everything. The top row
    // is free on both surfaces (popups start at y >= 1).
    render_status_toast(frame, area, app);

    // 4. Degrade the finished frame to the terminal's color support. Style
    // sites stay tier-unaware; only styles whose quantized form would be
    // unusable consult `app.caps` directly (e.g. the selection background).
    app.caps.adapt_buffer(frame.buffer_mut());
}

/// Yellow toast with the active status message in the top-right corner.
/// Truncated with a trailing ellipsis if wider than the frame.
fn render_status_toast(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    use ratatui::style::{Color, Style};
    use ratatui::text::Span;
    use ratatui::widgets::Paragraph;

    let Some((msg, _)) = &app.status_message else {
        return;
    };
    if area.width < 5 || area.height < 1 {
        return;
    }
    let max = area.width as usize - 2;
    let text = if msg.chars().count() > max {
        let head: String = msg.chars().take(max - 1).collect();
        format!("{head}…")
    } else {
        msg.clone()
    };
    let width = text.chars().count() as u16 + 2;
    let rect = ratatui::layout::Rect::new(area.x + area.width - width, area.y, width, 1);
    let toast = Paragraph::new(Span::styled(
        format!(" {text} "),
        Style::default().fg(Color::Yellow),
    ));
    frame.render_widget(toast, rect);
}

fn render_help(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    use ratatui::layout::{Constraint, Layout, Margin};
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

    let accent = app.accent_color();

    let effective_mode = app.previous_mode.as_ref().unwrap_or(&app.mode);

    let insert_surface_v = insert_surface(app);
    let is_board_selector_base = app.editor.is_none()
        || matches!(effective_mode, AppMode::BoardSelector)
        || matches!(effective_mode, AppMode::Insert)
            && matches!(insert_surface_v, Some(InsertSurface::BoardSelector));

    let is_card_detail = matches!(effective_mode, AppMode::CardDetail)
        || (matches!(effective_mode, AppMode::Insert)
            && matches!(insert_surface_v, Some(InsertSurface::CardDetail)));

    // Help opened from inside a dialog (`?` → `Follow::Help`): the dialog
    // declares its own reference rows via `Dialog::help()`.
    let dialog_help = app.dialog.as_ref().and_then(|d| d.help());

    // Help opened over a live insert handler (`?` → `InsertOutcome::Help`).
    // A dialog takes precedence: a dialog over insert (e.g. the discard
    // prompt) shows its own help page.
    let insert_handler = if dialog_help.is_none() {
        app.insert.as_ref()
    } else {
        None
    };
    let is_editing_desc = insert_handler
        .map(|h| {
            h.as_any()
                .downcast_ref::<crate::insert::markdown_editor::MarkdownEditor>()
                .is_some()
        })
        .unwrap_or(false);
    let is_date_picker = insert_handler
        .map(|h| {
            h.as_any()
                .downcast_ref::<crate::insert::date_picker::DatePicker>()
                .is_some()
        })
        .unwrap_or(false);

    let title = if let Some(dh) = &dialog_help {
        dh.title
    } else if is_editing_desc {
        " Help — Description Editor "
    } else if is_date_picker {
        " Help — Due Date "
    } else if is_card_detail {
        " Help — Card Detail "
    } else if is_board_selector_base {
        " Help — Board Selector "
    } else {
        " Help — Board View "
    };

    type HelpSection = (&'static str, Vec<(&'static str, &'static str)>);

    fn keymap_sections<A: Copy>(
        map: &[crate::input::keymap::Binding<A>],
        sections: &[&'static str],
    ) -> Vec<HelpSection> {
        sections
            .iter()
            .map(|&s| (s, crate::input::keymap::help_rows(map, s)))
            .collect()
    }

    let (left_secs, right_secs): (Vec<HelpSection>, Vec<HelpSection>) =
        if let Some(dh) = &dialog_help {
            (vec![("Keys", dh.rows.clone())], vec![])
        } else if is_editing_desc {
            // Description-editor keys live on the MarkdownEditor handler, not a
            // mode keymap — documented here directly.
            (
                vec![
                    (
                        "Edit",
                        vec![
                            ("ctrl+s", "save"),
                            ("ctrl+q", "save and close"),
                            ("esc", "cancel"),
                            ("ctrl+z / ctrl+y", "undo / redo"),
                            ("ctrl+c", "copy selection"),
                            ("ctrl+x", "cut selection"),
                            ("ctrl+v", "paste"),
                            ("shift+←↑↓→", "select text"),
                        ],
                    ),
                    (
                        "Format",
                        vec![
                            ("ctrl+b", "toggle bold (**text**)"),
                            ("ctrl+e", "toggle italic (*text*)"),
                            ("ctrl+k", "inline code (`text`)"),
                            ("ctrl+l", "insert list item (- )"),
                            ("ctrl+n", "footnote / jump back"),
                            ("ctrl+n (selected)", "name it after the text"),
                            ("ctrl+p", "toggle paste mode"),
                        ],
                    ),
                    (
                        "Lists",
                        vec![
                            ("enter", "auto-continue list"),
                            ("tab", "nest line(s)"),
                            ("shift+tab", "un-nest line(s)"),
                        ],
                    ),
                ],
                vec![
                    (
                        "Tables",
                        vec![
                            ("tab / shift+tab", "next/previous cell"),
                            ("enter", "row below / leave"),
                            ("alt+←↑↓→", "swap column / row"),
                            ("alt+shift+→ / ↓", "add column / row"),
                            ("alt+shift+← / ↑", "delete column / row"),
                            ("alt+a", "cycle alignment"),
                            ("ctrl+t", "insert table"),
                            ("ctrl+t, r / c", "add row / column"),
                            ("ctrl+t, R / C", "add before"),
                            ("ctrl+t, x / X", "delete row / column"),
                            ("ctrl+t, T", "delete table"),
                            ("ctrl+t, ←↑↓→", "swap row / column"),
                            ("ctrl+t, a", "cycle alignment"),
                            ("ctrl+t, t", "transpose matrix"),
                        ],
                    ),
                    (
                        "Alerts (in a quote)",
                        vec![
                            ("ctrl+g, n t i", "note / tip / important"),
                            ("ctrl+g, c w", "caution / warning"),
                            ("ctrl+g, x", "plain quote again"),
                        ],
                    ),
                ],
            )
        } else if is_date_picker {
            // Date-picker keys live on the DatePicker handler, not a mode
            // keymap — documented here directly.
            (
                vec![(
                    "Calendar",
                    vec![
                        ("← / →", "previous/next day"),
                        ("↑ / ↓", "previous/next week"),
                        ("pgup / pgdn", "previous/next month"),
                        ("shift+pgup/pgdn", "previous/next year"),
                        (", / .", "previous/next year"),
                        ("home / end", "first/last of month"),
                        ("t", "today"),
                    ],
                )],
                vec![
                    (
                        "Input",
                        vec![
                            ("0-9 / -", "type date (YYYY-MM-DD)"),
                            ("ctrl+u", "clear (remove due date)"),
                        ],
                    ),
                    ("File", vec![("enter", "save"), ("esc", "cancel")]),
                ],
            )
        } else if is_card_detail {
            let map = crate::input::card_detail_input::KEYMAP;
            (
                keymap_sections(map, &["Card", "Checklist"]),
                keymap_sections(map, &["Labels & Due", "View", "App"]),
            )
        } else if is_board_selector_base {
            let map = crate::input::board_selector_input::KEYMAP;
            (
                keymap_sections(map, &["Navigation"]),
                keymap_sections(map, &["Actions", "App"]),
            )
        } else {
            let map = crate::input::normal::KEYMAP;
            (
                keymap_sections(map, &["Navigation", "Card", "Move"]),
                keymap_sections(
                    map,
                    &["Lists", "Search & Filter", "Labels & Due", "View", "App"],
                ),
            )
        };

    // Per-column content width: longest key + 2-space gap + longest description
    // (or longest section header, if wider).
    fn column_metrics(secs: &[HelpSection]) -> (usize, usize) {
        let key_w = secs
            .iter()
            .flat_map(|(_, rows)| rows.iter().map(|(k, _)| k.chars().count()))
            .max()
            .unwrap_or(0);
        let desc_w = secs
            .iter()
            .flat_map(|(_, rows)| rows.iter().map(|(_, d)| d.chars().count()))
            .max()
            .unwrap_or(0);
        let header_w = secs
            .iter()
            .map(|(h, _)| h.chars().count())
            .max()
            .unwrap_or(0);
        let col_w = (key_w + 2 + desc_w).max(header_w);
        (key_w, col_w)
    }

    fn render_column(secs: &[HelpSection], key_w: usize, accent: Color) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        for (i, (title, rows)) in secs.iter().enumerate() {
            if i > 0 {
                lines.push(Line::raw(""));
            }
            lines.push(
                Line::from(Span::styled(
                    (*title).to_string(),
                    Style::default().fg(accent).add_modifier(Modifier::BOLD),
                ))
                .centered(),
            );
            for &(key, action) in rows {
                lines.push(Line::from(vec![
                    Span::styled(format!("{key:>key_w$}"), Style::default().fg(Color::Yellow)),
                    Span::raw("  "),
                    Span::raw(action.to_string()),
                ]));
            }
        }
        lines
    }

    const OUTER_MARGIN: usize = 2;
    const COL_GAP: usize = 4;

    let (left_key_w, left_col_w) = column_metrics(&left_secs);
    let (right_key_w, right_col_w) = column_metrics(&right_secs);
    let left = render_column(&left_secs, left_key_w, accent);
    let right = render_column(&right_secs, right_key_w, accent);

    // The popup hugs its content: 2-space outer margins inside the borders,
    // 4-space gap between columns on two-column pages, widened if needed for
    // the title or bottom border text.
    let content_w = if right_secs.is_empty() {
        left_col_w
    } else {
        left_col_w + COL_GAP + right_col_w
    };
    let inner_w = OUTER_MARGIN * 2 + content_w;
    let title_w = title.chars().count();
    let footer_w = " Esc:close ".len() + app.version.chars().count() + 2;
    let width = ((inner_w.max(title_w).max(footer_w) + 2) as u16)
        .min(area.width.saturating_sub(4))
        .max(20);
    let height = (left.len().max(right.len()) as u16 + 2)
        .min(area.height.saturating_sub(4))
        .max(5);
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup = ratatui::layout::Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(title)
        .title_bottom(Line::from(Span::styled(
            " Esc:close ",
            Style::default().fg(Color::DarkGray),
        )))
        .title_bottom(
            Line::from(Span::styled(
                format!(" {} ", app.version),
                Style::default().fg(Color::DarkGray),
            ))
            .right_aligned(),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent));
    let content = block
        .inner(popup)
        .inner(Margin::new(OUTER_MARGIN as u16, 0));
    frame.render_widget(block, popup);

    let columns = if right_secs.is_empty() {
        Layout::horizontal([Constraint::Fill(1), Constraint::Length(0)]).split(content)
    } else {
        Layout::horizontal([
            Constraint::Fill(left_col_w as u16),
            Constraint::Fill(right_col_w as u16),
        ])
        .spacing(COL_GAP as u16)
        .split(content)
    };

    frame.render_widget(Paragraph::new(left).wrap(Wrap { trim: false }), columns[0]);
    frame.render_widget(Paragraph::new(right).wrap(Wrap { trim: false }), columns[1]);
}

#[cfg(test)]
mod tests {
    use crate::app::{App, AppMode};
    use crate::insert::InsertSurface;

    /// Every section used by a keymap must be listed in the help layout in
    /// `render_help`, otherwise its bindings silently vanish from the
    /// overlay. Keep these lists in sync with `render_help`.
    #[test]
    fn help_layout_covers_all_keymap_sections() {
        fn sections<A: Copy>(map: &[crate::input::keymap::Binding<A>]) -> Vec<&'static str> {
            let mut secs: Vec<&'static str> = Vec::new();
            for b in map {
                if !secs.contains(&b.section) {
                    secs.push(b.section);
                }
            }
            secs
        }

        let layouts: &[(&[&str], Vec<&'static str>)] = &[
            (
                &[
                    "Navigation",
                    "Card",
                    "Move",
                    "Lists",
                    "Search & Filter",
                    "Labels & Due",
                    "View",
                    "App",
                ],
                sections(crate::input::normal::KEYMAP),
            ),
            (
                &["Card", "Checklist", "Labels & Due", "View", "App"],
                sections(crate::input::card_detail_input::KEYMAP),
            ),
            (
                &["Navigation", "Actions", "App"],
                sections(crate::input::board_selector_input::KEYMAP),
            ),
        ];
        for (layout, used) in layouts {
            for sec in used {
                assert!(
                    layout.contains(sec),
                    "keymap section '{sec}' missing from help layout"
                );
            }
        }
    }

    /// Smoke test: BoardSelector base when no board loaded.
    #[test]
    fn test_render_completeness() {
        crate::test_support::with_temp_dir(|| {
            let mut app = App::new(None).unwrap();
            app.mode = AppMode::Insert;
            // No board loaded → always BoardSelector base.
            assert!(app.editor.is_none());

            // With board loaded, only board-name insert handlers force the
            // selector background; other handlers use the BoardView.
            let board_meta = crate::model::board::BoardMeta::new("Test".into());
            app.editor = Some(crate::board_editor::BoardEditor::from_loaded(
                crate::app::LoadedBoard {
                    meta: board_meta,
                    lists: vec![],
                    cards: std::collections::HashMap::new(),
                    selected_list: 0,
                    selected_card: vec![],
                    scroll_offset: vec![],
                    label_filter: None,
                    detail_item_idx: 0,
                    detail_scroll: 0,
                    detail_max_scroll: std::cell::Cell::new(0),
                },
            ));

            // NewBoardName → BoardSelector surface
            app.insert = Some(Box::new(crate::insert::line_editor::NewBoardName::new()));
            assert_eq!(
                app.insert.as_ref().unwrap().surface(),
                InsertSurface::BoardSelector
            );

            // NewCardTitle → BoardView surface
            app.insert = Some(Box::new(crate::insert::line_editor::NewCardTitle::new()));
            assert_eq!(
                app.insert.as_ref().unwrap().surface(),
                InsertSurface::BoardView
            );
        });
    }
}
