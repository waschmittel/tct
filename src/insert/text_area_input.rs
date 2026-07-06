//! Multi-line text-area primitive backing the description editor.
//!
//! Wraps `ratatui_textarea::TextArea` and tracks the original text so we
//! can detect whether the user has unsaved changes. Soft wrap and wrapped
//! cursor navigation come from the textarea itself (`WrapMode::WordOrGlyph`);
//! the widget is rendered directly (frameless, inside the card detail
//! popup), so it also owns scrolling and the current-line highlight.

use ratatui::style::{Color, Style};
use ratatui_textarea::{TextArea, WrapMode};

pub struct TextAreaInput {
    pub textarea: TextArea<'static>,
    pub original: String,
}

impl TextAreaInput {
    pub fn from_initial(initial: &str) -> Self {
        let lines: Vec<String> = initial.split('\n').map(|s| s.to_string()).collect();
        let mut textarea = TextArea::new(lines);
        textarea.set_wrap_mode(WrapMode::WordOrGlyph);
        textarea.set_cursor_line_style(Style::default().bg(Color::Rgb(30, 30, 40)));
        textarea.set_style(Style::default().fg(Color::White));
        Self {
            textarea,
            original: initial.to_string(),
        }
    }

    pub fn current_text(&self) -> String {
        self.textarea.lines().join("\n")
    }

    pub fn is_modified(&self) -> bool {
        self.current_text() != self.original
    }
}
