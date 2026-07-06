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
            .map(|d| matches!(d.background(), crate::dialog::DialogBackground::BoardSelector))
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
            dialog::render(frame, area, app);
        }
        _ => {
            // Help may need card-detail as an intermediate layer.
            if let AppMode::Help = app.mode {
                let prev_card_detail = matches!(effective_mode, AppMode::CardDetail);
                let prev_insert_card_detail = matches!(effective_mode, AppMode::Insert)
                    && matches!(insert_surface(app), Some(InsertSurface::CardDetail));
                if prev_card_detail || prev_insert_card_detail {
                    card_detail::render(frame, area, app);
                }
            }
        }
    }

    // 3. Transient status toast, top-right, above everything. The top row
    // is free on both surfaces (popups start at y >= 1).
    render_status_toast(frame, area, app);
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
    let rect = ratatui::layout::Rect::new(
        area.x + area.width - width,
        area.y,
        width,
        1,
    );
    let toast = Paragraph::new(Span::styled(
        format!(" {text} "),
        Style::default().fg(Color::Yellow),
    ));
    frame.render_widget(toast, rect);
}

fn render_help(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    use ratatui::layout::{Constraint, Layout};
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

    let accent = app.accent_color();

    let width = 90u16.min(area.width.saturating_sub(4)).max(40);
    let height = 32u16.min(area.height.saturating_sub(4)).max(10);
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup = ratatui::layout::Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup);

    let effective_mode = app.previous_mode.as_ref().unwrap_or(&app.mode);

    let insert_surface_v = insert_surface(app);
    let is_board_selector_base = app.editor.is_none()
        || matches!(effective_mode, AppMode::BoardSelector)
        || matches!(effective_mode, AppMode::Insert)
            && matches!(insert_surface_v, Some(InsertSurface::BoardSelector));

    let is_card_detail = matches!(effective_mode, AppMode::CardDetail)
        || (matches!(effective_mode, AppMode::Insert)
            && matches!(insert_surface_v, Some(InsertSurface::CardDetail)));

    // Description editor: previous mode is `Insert` and handler is the
    // MarkdownEditor (titled "Edit Description").
    let is_editing_desc = matches!(effective_mode, AppMode::Insert)
        && app
            .insert
            .as_ref()
            .map(|h| h.title() == "Edit Description")
            .unwrap_or(false);

    // Help opened from inside a dialog (`?` → `Follow::Help`): the dialog
    // declares its own reference rows via `Dialog::help()`.
    let dialog_help = app.dialog.as_ref().and_then(|d| d.help());

    let title = if let Some(dh) = &dialog_help {
        dh.title
    } else if is_editing_desc {
        " Help — Description Editor "
    } else if is_card_detail {
        " Help — Card Detail "
    } else if is_board_selector_base {
        " Help — Board Selector "
    } else {
        " Help — Board View "
    };

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
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let columns = Layout::horizontal([
        Constraint::Percentage(50),
        Constraint::Percentage(50),
    ])
    .spacing(2)
    .split(inner);

    let header = |s: &str| -> Line<'static> {
        Line::from(Span::styled(
            s.to_string(),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ))
    };
    let row = |key: &str, action: &str| -> Line<'static> {
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                format!("{:<16}", key),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw(action.to_string()),
        ])
    };

    // One column of help lines from a keymap: section headers + their rows.
    fn keymap_column<A: Copy>(
        map: &[crate::input::keymap::Binding<A>],
        sections: &[&str],
        header: &dyn Fn(&str) -> Line<'static>,
        row: &dyn Fn(&str, &str) -> Line<'static>,
    ) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        for (i, section) in sections.iter().enumerate() {
            if i > 0 {
                lines.push(Line::raw(""));
            }
            lines.push(header(section));
            for (keys, help) in crate::input::keymap::help_rows(map, section) {
                lines.push(row(keys, help));
            }
        }
        lines
    }

    let (left, right): (Vec<Line>, Vec<Line>) = if let Some(dh) = &dialog_help {
        (
            std::iter::once(header("Keys"))
                .chain(dh.rows.iter().map(|(k, h)| row(k, h)))
                .collect(),
            vec![],
        )
    } else if is_editing_desc {
        // Description-editor keys live on the MarkdownEditor handler, not a
        // mode keymap — documented here directly.
        (
            vec![
                header("File"),
                row("Ctrl+S", "Save"),
                row("Esc", "Cancel"),
                row("Ctrl+Z / Ctrl+Y", "Undo / redo"),
                Line::raw(""),
                header("Format"),
                row("Ctrl+B", "Bold (**text**)"),
                row("Ctrl+I", "Italic (*text*)"),
                row("Ctrl+K", "Inline code (`text`)"),
                row("Ctrl+L", "Insert list item (- )"),
            ],
            vec![
                header("Lists"),
                row("Enter", "Auto-continue list"),
                row("Tab", "Nest item"),
                row("Shift+Tab", "Un-nest item"),
                Line::raw(""),
                header("Navigation"),
                row("Up / Down", "Move by visual line"),
            ],
        )
    } else if is_card_detail {
        let map = crate::input::card_detail_input::KEYMAP;
        (
            keymap_column(map, &["Card", "Checklist"], &header, &row),
            keymap_column(map, &["Labels & Due", "App"], &header, &row),
        )
    } else if is_board_selector_base {
        let map = crate::input::board_selector_input::KEYMAP;
        (
            keymap_column(map, &["Navigation"], &header, &row),
            keymap_column(map, &["Actions", "App"], &header, &row),
        )
    } else {
        let map = crate::input::normal::KEYMAP;
        (
            keymap_column(map, &["Navigation", "Card", "Move"], &header, &row),
            keymap_column(
                map,
                &["Lists", "Search & Filter", "Labels & Due", "App"],
                &header,
                &row,
            ),
        )
    };

    frame.render_widget(
        Paragraph::new(left).wrap(Wrap { trim: false }),
        columns[0],
    );
    frame.render_widget(
        Paragraph::new(right).wrap(Wrap { trim: false }),
        columns[1],
    );
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
                &["Navigation", "Card", "Move", "Lists", "Search & Filter", "Labels & Due", "App"],
                sections(crate::input::normal::KEYMAP),
            ),
            (
                &["Card", "Checklist", "Labels & Due", "App"],
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
        let mut app = App::new(None).unwrap();
        app.mode = AppMode::Insert;
        // No board loaded → always BoardSelector base.
        assert!(app.editor.is_none());

        // With board loaded, only board-name insert handlers force the
        // selector background; other handlers use the BoardView.
        let board_meta = crate::model::board::BoardMeta::new("Test".into());
        app.editor = Some(crate::board_editor::BoardEditor::from_loaded(crate::app::LoadedBoard {
            meta: board_meta,
            lists: vec![],
            cards: std::collections::HashMap::new(),
            selected_list: 0,
            selected_card: vec![],
            scroll_offset: vec![],
            detail_item_idx: 0,
            detail_scroll: 0,
        }));

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
    }
}
