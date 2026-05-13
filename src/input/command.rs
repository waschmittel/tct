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
