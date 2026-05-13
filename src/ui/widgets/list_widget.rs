use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders};
use ratatui::Frame;

use crate::app::App;
use crate::model::list::CardList;

use super::card_widget;

const CARD_HEIGHT: u16 = 4;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    list: &CardList,
    list_index: usize,
    is_selected: bool,
    app: &App,
) {
    let board = match &app.board {
        Some(b) => b,
        None => return,
    };

    let border_style = if is_selected {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let title_style = if is_selected {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    let active_count = list
        .card_ids
        .iter()
        .filter(|id| {
            board
                .cards
                .get(*id)
                .map(|c| !c.archived)
                .unwrap_or(false)
        })
        .count();

    let block = Block::default()
        .title_top(ratatui::text::Line::styled(
            format!(" {} ({}) ", list.name, active_count),
            title_style,
        ))
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let visible_cards: Vec<&str> = list
        .card_ids
        .iter()
        .filter(|id| {
            board
                .cards
                .get(*id)
                .map(|c| !c.archived)
                .unwrap_or(false)
        })
        .map(|id| id.as_str())
        .collect();

    if visible_cards.is_empty() {
        let empty = ratatui::widgets::Paragraph::new(ratatui::text::Span::styled(
            " (empty)",
            Style::default().fg(Color::DarkGray),
        ));
        frame.render_widget(empty, inner);
        return;
    }

    let max_visible = (inner.height / CARD_HEIGHT) as usize;
    let selected_card_idx = board.selected_card.get(list_index).copied().unwrap_or(0);

    // Compute scroll so selected card is always visible
    let mut scroll = board.scroll_offset.get(list_index).copied().unwrap_or(0);
    if max_visible > 0 {
        if selected_card_idx < scroll {
            scroll = selected_card_idx;
        } else if selected_card_idx >= scroll + max_visible {
            scroll = selected_card_idx - max_visible + 1;
        }
    }

    for (vi, ci) in (scroll..visible_cards.len()).enumerate() {
        if vi >= max_visible {
            break;
        }

        let card_id = visible_cards[ci];
        if let Some(card) = board.cards.get(card_id) {
            let card_area = Rect {
                x: inner.x,
                y: inner.y + (vi as u16 * CARD_HEIGHT),
                width: inner.width,
                height: CARD_HEIGHT.min(inner.height - vi as u16 * CARD_HEIGHT),
            };

            let is_card_selected = is_selected && ci == selected_card_idx;
            let dimmed = app.search_active && !card.matches_search(&app.search_query);
            let grabbed = board
                .grabbed_card
                .as_ref()
                .map(|g| g == card_id)
                .unwrap_or(false);

            card_widget::render(frame, card_area, card, is_card_selected, dimmed, grabbed);
        }
    }

    // Scroll indicators
    if scroll > 0 {
        let indicator = ratatui::widgets::Paragraph::new(
            ratatui::text::Span::styled("▲ more", Style::default().fg(Color::DarkGray)),
        );
        // Place at top-right of inner
        let ind_area = Rect::new(inner.x + inner.width.saturating_sub(7), inner.y, 7, 1);
        frame.render_widget(indicator, ind_area);
    }
    if scroll + max_visible < visible_cards.len() {
        let remaining = visible_cards.len() - scroll - max_visible;
        let indicator = ratatui::widgets::Paragraph::new(
            ratatui::text::Span::styled(
                format!("▼ +{remaining}"),
                Style::default().fg(Color::DarkGray),
            ),
        );
        let y_pos = inner.y + inner.height.saturating_sub(1);
        let ind_area = Rect::new(inner.x + inner.width.saturating_sub(7), y_pos, 7, 1);
        frame.render_widget(indicator, ind_area);
    }
}
