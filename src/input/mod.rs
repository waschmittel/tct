mod board_selector_input;
mod card_detail_input;
mod command;
mod dialog_input;
mod insert;
mod normal;

use crossterm::event::{KeyEvent, KeyEventKind};

use crate::app::{App, AppMode};

pub fn handle_input(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    if key.kind != KeyEventKind::Press {
        return Ok(());
    }

    match &app.mode {
        AppMode::BoardSelector => board_selector_input::handle(app, key),
        AppMode::Normal => normal::handle(app, key),
        AppMode::CardDetail => card_detail_input::handle(app, key),
        AppMode::Insert(_) => insert::handle(app, key),
        AppMode::Command => command::handle(app, key),
        AppMode::Dialog(_) => dialog_input::handle(app, key),
        AppMode::Help => {
            if matches!(key.code, crossterm::event::KeyCode::Esc | crossterm::event::KeyCode::Char('q')) {
                if let Some(prev) = app.previous_mode.take() {
                    app.mode = prev;
                } else if app.board.is_some() {
                    app.mode = AppMode::Normal;
                } else {
                    app.mode = AppMode::BoardSelector;
                }
            }
            Ok(())
        }
    }
}
