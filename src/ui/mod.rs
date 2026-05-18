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

use crate::app::{App, AppMode, InsertTarget};

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // 1. Determine and render the base background layer
    let effective_mode = if let AppMode::Help = app.mode {
        app.previous_mode.as_ref().unwrap_or(&app.mode)
    } else {
        &app.mode
    };

    let is_board_selector_base = app.board.is_none() || matches!(
        effective_mode,
        AppMode::BoardSelector
            | AppMode::Insert(InsertTarget::NewBoardName)
            | AppMode::Insert(InsertTarget::RenameBoard)
            | AppMode::Dialog(crate::app::DialogKind::ConfirmArchiveBoard)
            | AppMode::Dialog(crate::app::DialogKind::ArchivedBoards)
    );

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
        AppMode::CardDetail
        | AppMode::Insert(
            InsertTarget::EditCardTitle
            | InsertTarget::EditCardDescription
            | InsertTarget::NewChecklistItem
            | InsertTarget::EditChecklistItem
        ) => {
            card_detail::render(frame, area, app);
        }
        AppMode::Insert(InsertTarget::EditDueDate) => {
            if effective_mode == &AppMode::CardDetail {
                card_detail::render(frame, area, app);
            }
        }
        AppMode::Dialog(_kind) => {
            // Some dialogs are specific to BoardSelector and already rendered background above.
            // Some are specific to BoardView. dialog::render handles the specific popup content.
            dialog::render(frame, area, app);
        }
        _ => {
            // If we are in Help, we might still need to render CardDetail as an intermediate layer
            if let AppMode::Help = app.mode {
                if matches!(
                    effective_mode,
                    AppMode::CardDetail
                        | AppMode::Insert(
                            InsertTarget::EditCardTitle
                            | InsertTarget::EditCardDescription
                            | InsertTarget::NewChecklistItem
                            | InsertTarget::EditChecklistItem
                            | InsertTarget::EditDueDate
                        )
                ) {
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

    let is_board_selector_base = app.board.is_none() || matches!(
        effective_mode,
        AppMode::BoardSelector
            | AppMode::Insert(InsertTarget::NewBoardName)
            | AppMode::Insert(InsertTarget::RenameBoard)
            | AppMode::Dialog(crate::app::DialogKind::ConfirmArchiveBoard)
            | AppMode::Dialog(crate::app::DialogKind::ArchivedBoards)
    );

    let is_card_detail = matches!(
        effective_mode,
        AppMode::CardDetail
            | AppMode::Insert(
                InsertTarget::EditCardTitle
                    | InsertTarget::EditCardDescription
                    | InsertTarget::NewChecklistItem
                    | InsertTarget::EditChecklistItem
                    | InsertTarget::EditDueDate
            )
    );

    let is_editing_desc = matches!(
        effective_mode,
        AppMode::Insert(InsertTarget::EditCardDescription)
    );

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
                row("y", "Copy description"),
                row("Y", "Copy checklist (md)"),
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
                row("d", "Archive board"),
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
                row("d", "Delete card"),
                row("a", "Archive card"),
                row("v", "View archived"),
                Line::raw(""),
                header("Move"),
                row("Shift+Left/Right", "Move to adjacent list"),
                row("Shift+Up/Down", "Move within list"),
            ],
            vec![
                header("Lists"),
                row("N", "New list"),
                row("r", "Rename list"),
                row("D", "Delete list"),
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
    use crate::app::{App, InsertTarget, AppMode};

    #[test]
    fn test_render_completeness() {
        let mut app = App::new(None).unwrap();
        
        let all_targets = vec![
            InsertTarget::NewCardTitle,
            InsertTarget::EditCardTitle,
            InsertTarget::EditCardTitleInline,
            InsertTarget::EditCardDescription,
            InsertTarget::NewListName,
            InsertTarget::RenameList,
            InsertTarget::NewChecklistItem,
            InsertTarget::EditChecklistItem,
            InsertTarget::NewBoardName,
            InsertTarget::RenameBoard,
            InsertTarget::EditDueDate,
            InsertTarget::NewLabelName,
            InsertTarget::EditLabelName,
        ];

        // Case 1: No board loaded (BoardSelector background for ALL)
        for target in &all_targets {
            app.mode = AppMode::Insert(target.clone());
            
            let is_selector = app.board.is_none() || matches!(
                app.mode,
                AppMode::BoardSelector
                    | AppMode::Insert(InsertTarget::NewBoardName)
                    | AppMode::Insert(InsertTarget::RenameBoard)
                    | AppMode::Dialog(crate::app::DialogKind::ConfirmArchiveBoard)
                    | AppMode::Dialog(crate::app::DialogKind::ArchivedBoards)
            );

            assert!(is_selector, "When no board is loaded, base should be BoardSelector for target {:?}", target);
        }

        // Case 2: Board IS loaded (BoardView background for card-related targets)
        // Simulate a loaded board
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

        for target in all_targets {
            app.mode = AppMode::Insert(target.clone());
            
            let is_selector = app.board.is_none() || matches!(
                app.mode,
                AppMode::BoardSelector
                    | AppMode::Insert(InsertTarget::NewBoardName)
                    | AppMode::Insert(InsertTarget::RenameBoard)
                    | AppMode::Dialog(crate::app::DialogKind::ConfirmArchiveBoard)
                    | AppMode::Dialog(crate::app::DialogKind::ArchivedBoards)
            );

            if target == InsertTarget::NewBoardName || target == InsertTarget::RenameBoard {
                assert!(is_selector, "Target {:?} should always use BoardSelector background", target);
            } else {
                assert!(!is_selector, "Target {:?} should use BoardView background when board is loaded", target);
            }
        }
    }
}
