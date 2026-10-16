//! Multi-line text-area primitive backing the description editor.
//!
//! Wraps `tui_textarea::TextArea` and tracks the original text so we
//! can detect whether the user has unsaved changes. Soft wrap and wrapped
//! cursor navigation come from the textarea itself (`WrapMode::WordOrGlyph`);
//! the widget is rendered directly (frameless, inside the card detail
//! popup), so it also owns scrolling and the current-line highlight.

use ratatui::style::{Color, Style};
use tui_textarea::{TextArea, WrapMode};

pub struct TextAreaInput {
    pub textarea: TextArea<'static>,
    pub original: String,
}

impl TextAreaInput {
    pub fn from_initial(initial: &str) -> Self {
        let lines: Vec<String> = initial.split('\n').map(|s| s.to_string()).collect();
        let mut textarea = TextArea::new(lines);
        textarea.set_wrap_mode(WrapMode::WordOrGlyph);
        textarea.set_cursor_line_style(Style::default());
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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    /// `width` is the wrap width: the widget keeps one more column for the caret.
    fn render_rows(text: &str, width: u16, cursor_end: bool) -> Vec<String> {
        let mut input = TextAreaInput::from_initial(text);
        if cursor_end {
            input.textarea.move_cursor(tui_textarea::CursorMove::Bottom);
            input.textarea.move_cursor(tui_textarea::CursorMove::End);
        }
        let mut terminal = Terminal::new(TestBackend::new(width + 1, 4)).unwrap();
        terminal.draw(|f| f.render_widget(&input.textarea, f.area())).unwrap();
        let buf = terminal.backend().buffer().clone();
        (0..4)
            .map(|y| (0..width + 1).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>())
            .collect()
    }

    #[test]
    fn space_after_a_word_ending_at_the_edge_does_not_start_the_next_row() {
        let rows = render_rows("0123456789 next words", 10, false);
        assert_eq!(rows[0].trim_end(), "0123456789");
        assert_eq!(rows[1].trim_end(), "next words");
    }

    #[test]
    fn spaces_inside_a_row_are_kept() {
        let rows = render_rows("ab cd ef", 20, false);
        assert_eq!(rows[0].trim_end(), "ab cd ef");
    }

    #[test]
    fn a_word_wider_than_the_row_still_splits() {
        let rows = render_rows("abcdefghijkl", 10, false);
        assert_eq!(rows[0].trim_end(), "abcdefghij");
        assert_eq!(rows[1].trim_end(), "kl");
    }

    #[test]
    fn cursor_after_the_hanging_space_does_not_panic() {
        let rows = render_rows("0123456789 ", 10, true);
        assert_eq!(rows[0].trim_end(), "0123456789");
    }

    #[test]
    fn current_line_has_no_highlight() {
        let input = TextAreaInput::from_initial("first\nsecond");
        assert_eq!(input.textarea.cursor_line_style(), Style::default());
    }
}
