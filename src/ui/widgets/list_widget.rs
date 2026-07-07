use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders};
use ratatui::Frame;

use crate::app::App;
use crate::model::card::Card;
use crate::model::label::Label;
use crate::model::list::CardList;

use super::card_widget;

/// Returns the rendered height (including borders) for a card in a list
/// column. Counts the exact lines the card widget renders (styles don't
/// affect the count), so height and content can't drift apart.
pub fn card_height(card: &Card, board_labels: &[Label], card_inner_width: u16) -> u16 {
    let inner_lines =
        card_widget::card_lines(card, board_labels, card_inner_width, Style::default(), false)
            .len() as u16;
    2 + inner_lines // 2 for borders
}

pub fn render(
    frame: &mut Frame,
    area: Rect,
    list: &CardList,
    list_index: usize,
    is_selected: bool,
    app: &App,
) {
    let board = match app.board() {
        Some(b) => b,
        None => return,
    };

    let accent = app.accent_color();

    let border_style = if is_selected {
        Style::default().fg(accent)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let title_style = if is_selected {
        Style::default().fg(accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    let active_count = board.visible_cards(list_index, None).len();

    let count_label = if app.search_active {
        let filtered_count = board.visible_cards(list_index, app.search()).len();
        format!("{filtered_count}/{active_count}")
    } else {
        format!("{active_count}")
    };

    let block = Block::default()
        .title_top(ratatui::text::Line::styled(
            format!(" {} ({}) ", list.name, count_label),
            title_style,
        ))
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let selected_card_idx = board.selected_card.get(list_index).copied().unwrap_or(0);

    let selected_card_id = board
        .visible_cards(list_index, None)
        .get(selected_card_idx)
        .map(|&i| list.card_ids[i].as_str());

    let visible_cards: Vec<&str> = board
        .visible_cards(list_index, app.search())
        .into_iter()
        .map(|i| list.card_ids[i].as_str())
        .collect();

    if visible_cards.is_empty() {
        let empty = ratatui::widgets::Paragraph::new(ratatui::text::Span::styled(
            " (empty)",
            Style::default().fg(Color::DarkGray),
        ));
        frame.render_widget(empty, inner);
        return;
    }

    // card_inner_width: inner area of each card widget (subtracting card borders)
    let card_inner_width = inner.width.saturating_sub(2);

    let get_height = |card_id: &str| -> u16 {
        board
            .cards
            .get(card_id)
            .map(|c| card_height(c, &board.meta.labels, card_inner_width))
            .unwrap_or(3)
    };

    // Count how many cards fit starting from a given index
    let fitting_from = |start: usize| -> usize {
        let mut h = 0u16;
        let mut count = 0usize;
        for &card_id in visible_cards.iter().skip(start) {
            let ch = get_height(card_id);
            if h + ch > inner.height {
                break;
            }
            h += ch;
            count += 1;
        }
        count
    };

    let selected_pos_in_filtered = selected_card_id
        .and_then(|sid| visible_cards.iter().position(|&id| id == sid))
        .unwrap_or(0);

    let mut scroll = board.scroll_offset.get(list_index).copied().unwrap_or(0);
    scroll = scroll.min(visible_cards.len().saturating_sub(1));

    let mut max_visible = fitting_from(scroll);

    if selected_pos_in_filtered < scroll {
        scroll = selected_pos_in_filtered;
        max_visible = fitting_from(scroll);
    } else if max_visible == 0 || selected_pos_in_filtered >= scroll + max_visible {
        // Find the latest scroll so selected fits as the last visible card
        let mut h = 0u16;
        let mut count = 0usize;
        for ci in (0..=selected_pos_in_filtered).rev() {
            let ch = get_height(visible_cards[ci]);
            if h + ch > inner.height {
                break;
            }
            h += ch;
            count += 1;
        }
        scroll = selected_pos_in_filtered.saturating_sub(count.saturating_sub(1));
        max_visible = fitting_from(scroll);
    }

    let mut y_offset = 0u16;
    let mut rendered_count = 0usize;

    for &card_id in visible_cards.iter().skip(scroll) {
        if let Some(card) = board.cards.get(card_id) {
            let ch = card_height(card, &board.meta.labels, card_inner_width);
            if y_offset + ch > inner.height {
                break;
            }

            let card_area = Rect {
                x: inner.x,
                y: inner.y + y_offset,
                width: inner.width,
                height: ch,
            };

            let is_card_selected = is_selected && Some(card_id) == selected_card_id;
            card_widget::render(
                frame,
                card_area,
                card,
                is_card_selected,
                false,
                &board.meta.labels,
                accent,
                app.caps,
            );

            y_offset += ch;
            rendered_count += 1;
        }
    }

    // Scroll indicators (accent-colored so users notice hidden cards)
    let scroll_above = scroll;
    if scroll_above > 0 {
        let label = format!("▲ +{scroll_above}");
        let w = label.chars().count() as u16 + 1;
        let indicator = ratatui::widgets::Paragraph::new(
            ratatui::text::Span::styled(
                label,
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ),
        );
        let ind_area = Rect::new(inner.x + inner.width.saturating_sub(w), inner.y, w, 1);
        frame.render_widget(indicator, ind_area);
    }
    if scroll + rendered_count < visible_cards.len() {
        let remaining = visible_cards.len() - scroll - rendered_count;
        let label = format!("▼ +{remaining}");
        let w = label.chars().count() as u16 + 1;
        let indicator = ratatui::widgets::Paragraph::new(
            ratatui::text::Span::styled(
                label,
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ),
        );
        let y_pos = inner.y + inner.height.saturating_sub(1);
        let ind_area = Rect::new(inner.x + inner.width.saturating_sub(w), y_pos, w, 1);
        frame.render_widget(indicator, ind_area);
    }

    let _ = max_visible; // used in scroll adjustment above
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_height_with_info_no_labels() {
        let mut card = Card::new("Title".into());
        card.description = "desc".into();
        // title(1) + info(1) + borders(2) = 4
        assert_eq!(card_height(&card, &[], 20), 4);
    }

    #[test]
    fn card_height_title_only() {
        let card = Card::new("Title".into());
        // title(1) + borders(2) = 3
        assert_eq!(card_height(&card, &[], 20), 3);
    }

    #[test]
    fn card_height_counts_wrapped_title_and_labels() {
        let label = Label::new("a rather long label".into(), crate::model::label::LabelColor::Red);
        let mut card = Card::new("one two three four".into());
        card.label_ids = vec![label.id.clone()];
        card.description = "desc".into();
        // width 9: title wraps to 3 ("one two"/"three"/"four"),
        // "[a rather long label]" (21) cut to one line, info(1), borders(2)
        assert_eq!(card_height(&card, &[label], 9), 7);
    }
}
