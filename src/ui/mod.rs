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
    use ratatui::style::{Color, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

    let width = 60u16.min(area.width.saturating_sub(4));
    let height = 30u16.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup = ratatui::layout::Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup);
    let block = Block::default()
        .title(" Help (Esc to close) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let mut help_text = Vec::new();

    let effective_mode = app.previous_mode.as_ref().unwrap_or(&app.mode);

    let is_board_selector_base = app.board.is_none() || matches!(
        effective_mode,
        AppMode::BoardSelector
            | AppMode::Insert(InsertTarget::NewBoardName)
            | AppMode::Insert(InsertTarget::RenameBoard)
            | AppMode::Dialog(crate::app::DialogKind::ConfirmArchiveBoard)
            | AppMode::Dialog(crate::app::DialogKind::ArchivedBoards)
    );

    if is_board_selector_base {
        help_text.extend(vec![
            Line::from(Span::styled("Board Selector", Style::default().fg(Color::Cyan))),
            Line::raw("  Up/Down        Navigate boards"),
            Line::raw("  Shift+Up/Down  Reorder board up/down"),
            Line::raw("  Enter          Open board"),
            Line::raw("  n              New board"),
            Line::raw("  r              Rename board"),
            Line::raw("  c              Cycle board accent color"),
            Line::raw("  d              Archive board"),
            Line::raw("  v              View archived boards"),
            Line::raw("  q              Quit"),
        ]);
    } else if matches!(
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
        help_text.extend(vec![
            Line::from(Span::styled("Card Detail", Style::default().fg(Color::Cyan))),
            Line::raw("  t              Edit title"),
            Line::raw("  e              Edit description"),
            Line::raw("  y              Copy description"),
            Line::raw("  Up/Down        Navigate checklist items"),
            Line::raw("  Shift+Up/Down  Reorder checklist item"),
            Line::raw("  Space          Toggle checklist item"),
            Line::raw("  a              Add checklist item"),
            Line::raw("  Enter          Edit checklist item"),
            Line::raw("  x              Delete checklist item"),
            Line::raw("  l              Assign/remove labels"),
            Line::raw("  L              Manage labels"),
            Line::raw("  u              Set due date"),
            Line::raw("  U              Clear due date"),
            Line::raw("  Esc            Close"),
            Line::raw("  q              Quit"),
        ]);

        if matches!(effective_mode, AppMode::Insert(InsertTarget::EditCardDescription)) {
            help_text.extend(vec![
                Line::raw(""),
                Line::from(Span::styled("Description Editor", Style::default().fg(Color::Cyan))),
                Line::raw("  Ctrl+S     Save"),
                Line::raw("  Ctrl+Z/Y   Undo/redo"),
                Line::raw("  Ctrl+B     Bold (**text**)"),
                Line::raw("  Ctrl+I     Italic (*text*)"),
                Line::raw("  Ctrl+K     Code (`text`)"),
                Line::raw("  Ctrl+L     List item (- )"),
                Line::raw("  Up/Down    Move by visual (wrapped) line"),
                Line::raw("  Enter      Auto-continue lists"),
                Line::raw("  Esc        Cancel"),
            ]);
        }
    } else {
        // Default to Board View help
        help_text.extend(vec![
            Line::from(Span::styled("Board View", Style::default().fg(Color::Cyan))),
            Line::raw("  Left/Right         Switch lists"),
            Line::raw("  Up/Down            Navigate cards"),
            Line::raw("  Shift+Left/Right   Move card to adjacent list"),
            Line::raw("  Shift+Up/Down      Move card up/down within list"),
            Line::raw("  g/G                First/last card"),
            Line::raw("  Enter              Open card detail"),
            Line::raw("  e                  Quick-edit card title inline"),
            Line::raw("  y                  Copy card title"),
            Line::raw("  n                  New card"),
            Line::raw("  N                  New list"),
            Line::raw("  r                  Rename list"),
            Line::raw("  d                  Delete card"),
            Line::raw("  D                  Delete list"),
            Line::raw("  a                  Archive card"),
            Line::raw("  v                  View archived cards"),
            Line::raw("  </>                Reorder list left/right"),
            Line::raw("  /                  Search"),
            Line::raw("  f                  Filter by label"),
            Line::raw("  F                  Clear all filters"),
            Line::raw("  l                  Assign/remove labels"),
            Line::raw("  L                  Manage labels"),
            Line::raw("  u                  Set due date"),
            Line::raw("  U                  Clear due date"),
            Line::raw("  b                  Back to board selector"),
            Line::raw("  q                  Quit"),
        ]);
    }

    let paragraph = Paragraph::new(help_text).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
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
