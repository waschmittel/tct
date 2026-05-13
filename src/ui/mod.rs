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

    match &app.mode {
        AppMode::BoardSelector
        | AppMode::Insert(InsertTarget::NewBoardName)
        | AppMode::Dialog(crate::app::DialogKind::ConfirmDeleteBoard) => {
            board_selector::render(frame, area, app);

            if matches!(app.mode, AppMode::Dialog(crate::app::DialogKind::ConfirmDeleteBoard)) {
                dialog::render(frame, area, app);
            }
        }
        _ => {
            board_view::render(frame, area, app);

            if matches!(
                app.mode,
                AppMode::CardDetail
                    | AppMode::Insert(
                        InsertTarget::EditCardTitle
                            | InsertTarget::EditCardDescription
                            | InsertTarget::NewChecklistTitle
                            | InsertTarget::NewChecklistItem
                            | InsertTarget::EditChecklistItem
                            | InsertTarget::EditDueDate
                    )
            ) {
                card_detail::render(frame, area, app);
            }

            if matches!(
                app.mode,
                AppMode::Dialog(crate::app::DialogKind::ConfirmDeleteCard)
                    | AppMode::Dialog(crate::app::DialogKind::ConfirmDeleteList)
                    | AppMode::Dialog(crate::app::DialogKind::LabelPicker)
            ) {
                dialog::render(frame, area, app);
            }

            if matches!(app.mode, AppMode::Command) {
                search_bar::render(frame, area, app);
            }

            if matches!(app.mode, AppMode::Help) {
                render_help(frame, area);
            }
        }
    }
}

fn render_help(frame: &mut Frame, area: ratatui::layout::Rect) {
    use ratatui::style::{Color, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

    let width = 60u16.min(area.width.saturating_sub(4));
    let height = 34u16.min(area.height.saturating_sub(4));
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

    let help_text = vec![
        Line::from(Span::styled("Board View", Style::default().fg(Color::Cyan))),
        Line::raw("  h/l or Left/Right  Switch lists"),
        Line::raw("  j/k or Down/Up     Navigate cards"),
        Line::raw("  g/G                First/last card"),
        Line::raw("  Enter              Open card detail"),
        Line::raw("  n                  New card"),
        Line::raw("  N                  New list"),
        Line::raw("  e                  Quick-edit card title"),
        Line::raw("  r                  Rename list"),
        Line::raw("  d                  Delete card"),
        Line::raw("  D                  Delete list"),
        Line::raw("  a                  Archive card"),
        Line::raw("  m                  Move card to another list"),
        Line::raw("  J/K                Reorder card up/down"),
        Line::raw("  </> (Shift+,/.)    Reorder list left/right"),
        Line::raw("  /                  Search"),
        Line::raw("  f                  Filter by label"),
        Line::raw("  F                  Clear filters"),
        Line::raw("  b                  Back to board selector"),
        Line::raw("  q                  Quit"),
        Line::raw(""),
        Line::from(Span::styled("Card Detail", Style::default().fg(Color::Cyan))),
        Line::raw("  Tab        Cycle sections"),
        Line::raw("  t          Edit title"),
        Line::raw("  e          Edit field"),
        Line::raw("  Space      Toggle checklist item"),
        Line::raw("  a          Add checklist item"),
        Line::raw("  A          Add checklist"),
        Line::raw("  x          Delete checklist item"),
        Line::raw("  l          Toggle label (Labels tab)"),
        Line::raw("  u          Set due date"),
        Line::raw("  Esc        Close"),
        Line::raw(""),
        Line::from(Span::styled("Description Editor", Style::default().fg(Color::Cyan))),
        Line::raw("  Ctrl+S     Save"),
        Line::raw("  Ctrl+B     Bold (**text**)"),
        Line::raw("  Ctrl+I     Italic (*text*)"),
        Line::raw("  Ctrl+K     Code (`text`)"),
        Line::raw("  Ctrl+L     List item (- )"),
        Line::raw("  Ctrl+T     Table template"),
        Line::raw("  Esc        Cancel"),
    ];

    let paragraph = Paragraph::new(help_text).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}
