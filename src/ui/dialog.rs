//! Thin Dialog mode dispatcher — delegates to the active
//! `Box<dyn Dialog>` on `App`. Each dialog kind owns its own render
//! function inside `src/dialog/<kind>.rs`.

use ratatui::Frame;
use ratatui::layout::Rect;

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    if let Some(dialog) = &app.dialog {
        dialog.render(frame, area, app.board(), app.accent_color());
    }
}
