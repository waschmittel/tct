//! Multi-line text-area primitive backing the description editor.
//!
//! Wraps `ratatui_textarea::TextArea` and tracks the original text so we
//! can detect whether the user has unsaved changes.

use ratatui_textarea::TextArea;

pub struct TextAreaInput {
    pub textarea: TextArea<'static>,
    pub original: String,
    pub scroll: usize,
}

impl TextAreaInput {
    pub fn from_initial(initial: &str) -> Self {
        use ratatui::style::{Color, Style};
        let lines: Vec<String> = initial.split('\n').map(|s| s.to_string()).collect();
        let mut textarea = TextArea::new(lines);
        textarea.set_cursor_line_style(Style::default());
        textarea.set_style(Style::default().fg(Color::White));
        textarea.set_block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .title(" Edit Description "),
        );
        Self {
            textarea,
            original: initial.to_string(),
            scroll: 0,
        }
    }

    pub fn current_text(&self) -> String {
        self.textarea.lines().join("\n")
    }

    pub fn is_modified(&self) -> bool {
        self.current_text() != self.original
    }
}
