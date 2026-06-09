pub mod board_selector;
pub mod board_view;
pub mod card_detail;
pub mod dialog;
pub mod markdown;
pub mod search_bar;
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

    let is_board_selector_base = app.board.is_none()
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
    let is_board_selector_base = app.board.is_none()
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

    let title = if is_editing_desc {
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

    let (left, right): (Vec<Line>, Vec<Line>) = if is_editing_desc {
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
        (
            vec![
                header("Card"),
                row("t", "Edit title"),
                row("e", "Edit description"),
                row("PgUp / PgDn", "Scroll description"),
                row("y", "Copy description"),
                row("Y", "Copy checklist (md)"),
                row("h", "View change history"),
                Line::raw(""),
                header("Checklist"),
                row("Up / Down", "Navigate items"),
                row("Shift+Up/Down", "Reorder item"),
                row("Space", "Toggle item"),
                row("a", "Add item"),
                row("Enter", "Edit item"),
                row("x", "Delete item"),
            ],
            vec![
                header("Labels & Due"),
                row("l", "Assign / remove labels"),
                row("L", "Manage labels"),
                row("u", "Set due date"),
                row("U", "Clear due date"),
                Line::raw(""),
                header("App"),
                row("Esc", "Close"),
                row("q", "Quit"),
            ],
        )
    } else if is_board_selector_base {
        (
            vec![
                header("Navigation"),
                row("Up / Down", "Navigate boards"),
                row("Shift+Up/Down", "Reorder board"),
                row("Enter", "Open board"),
            ],
            vec![
                header("Actions"),
                row("n", "New board"),
                row("r", "Rename board"),
                row("c", "Cycle accent color"),
                row("a", "Archive board"),
                row("v", "View archived"),
                Line::raw(""),
                header("App"),
                row("?", "Help"),
                row("q", "Quit"),
            ],
        )
    } else {
        (
            vec![
                header("Navigation"),
                row("Left / Right", "Switch lists"),
                row("Up / Down", "Navigate cards"),
                row("g / G", "First / last card"),
                row("Enter", "Open card detail"),
                Line::raw(""),
                header("Card"),
                row("t", "Quick-edit title"),
                row("e", "Edit description"),
                row("y", "Copy title"),
                row("n", "New card"),
                row("a", "Archive card"),
                row("v", "View archived cards"),
                row("h", "View change history"),
                Line::raw(""),
                header("Move"),
                row("Shift+Left/Right", "Move to adjacent list"),
                row("Shift+Up/Down", "Move within list"),
            ],
            vec![
                header("Lists"),
                row("N", "New list"),
                row("r", "Rename list"),
                row("A", "Archive list"),
                row("V", "View archived lists"),
                row("< / >", "Reorder list"),
                Line::raw(""),
                header("Search & Filter"),
                row("/", "Search"),
                row("f", "Filter by label"),
                row("F", "Clear filters"),
                Line::raw(""),
                header("Labels & Due"),
                row("l", "Assign / remove labels"),
                row("L", "Manage labels"),
                row("u", "Set due date"),
                row("U", "Clear due date"),
                Line::raw(""),
                header("App"),
                row("b", "Back to selector"),
                row("?", "Help"),
                row("q", "Quit"),
            ],
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

    /// Smoke test: BoardSelector base when no board loaded.
    #[test]
    fn test_render_completeness() {
        let mut app = App::new(None).unwrap();
        app.mode = AppMode::Insert;
        // No board loaded → always BoardSelector base.
        assert!(app.board.is_none());

        // With board loaded, only board-name insert handlers force the
        // selector background; other handlers use the BoardView.
        let board_meta = crate::model::board::BoardMeta::new("Test".into());
        app.board = Some(crate::app::LoadedBoard {
            meta: board_meta,
            lists: vec![],
            cards: std::collections::HashMap::new(),
            selected_list: 0,
            selected_card: vec![],
            scroll_offset: vec![],
            detail_item_idx: 0,
            detail_scroll: 0,
        });

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
