use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::App;
use crate::command::MoveDir;

use super::keymap;
use actions::*;
pub(super) use actions::{toggle_created_date, toggle_updated_date};
pub use bindings::{Action, KEYMAP};

mod actions;
mod bindings;
#[cfg(test)]
mod tests;

pub fn handle(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    // Grab state: plain arrows move the grabbed card — usable on terminals
    // that never report Shift+arrows (Linux console). Every other key is
    // swallowed until the card is dropped, so the mode can't change
    // underneath an active grab.
    if app.grab_active {
        match key.code {
            KeyCode::Left => return move_card_in_direction(app, MoveDir::Left),
            KeyCode::Right => return move_card_in_direction(app, MoveDir::Right),
            KeyCode::Up => return move_card_in_direction(app, MoveDir::Up),
            KeyCode::Down => return move_card_in_direction(app, MoveDir::Down),
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('m') => {
                app.grab_active = false;
            }
            _ => {}
        }
        return Ok(());
    }

    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    let Some(action) = keymap::lookup(KEYMAP, key.code, shift) else {
        return Ok(());
    };
    run(app, action)
}
