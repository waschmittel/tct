use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{App, AppMode};

pub fn handle(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    match key.code {
        KeyCode::Esc => {
            app.search_active = false;
            app.search_query.clear();
            app.mode = AppMode::Normal;
        }
        KeyCode::Enter => {
            if app.search_query.is_empty() {
                app.search_active = false;
            } else {
                app.search_active = true;
                select_first_match(app);
            }
            app.mode = AppMode::Normal;
        }
        KeyCode::Backspace => {
            app.search_query.pop();
        }
        KeyCode::Char(c) => {
            app.search_query.push(c);
        }
        _ => {}
    }
    Ok(())
}

fn select_first_match(app: &mut App) {
    let board = match &mut app.board {
        Some(b) => b,
        None => return,
    };
    let query = &app.search_query;
    for li in 0..board.lists.len() {
        for (ci, card_id) in board.lists[li].card_ids.iter().enumerate() {
            if let Some(card) = board.cards.get(card_id)
                && !card.archived && card.matches_search(query, &board.meta.labels) {
                    board.selected_list = li;
                    board.selected_card[li] = ci;
                    return;
                }
        }
    }
}
